//! Deterministic dependency planning for the parametric engine (ADR 0046).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt;

/// Opaque identifier for one node in a dependency graph.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DependencyNode(u64);

impl DependencyNode {
    /// Creates an identifier. The owner of a graph defines what its numbers mean.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The identifier's numeric value.
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Why a dependency graph operation could not be planned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyGraphError {
    /// An edge or regeneration request named a node not in the graph.
    UnknownNode(DependencyNode),
    /// A node cannot depend on itself.
    SelfDependency(DependencyNode),
    /// An edge would make the graph cyclic.
    Cycle {
        /// The source node whose change would cause regeneration.
        prerequisite: DependencyNode,
        /// The node that depends on the prerequisite.
        dependent: DependencyNode,
    },
}

impl fmt::Display for DependencyGraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownNode(node) => write!(f, "unknown dependency node {}", node.value()),
            Self::SelfDependency(node) => {
                write!(
                    f,
                    "dependency node {} cannot depend on itself",
                    node.value()
                )
            }
            Self::Cycle {
                prerequisite,
                dependent,
            } => write!(
                f,
                "dependency from {} to {} would create a cycle",
                prerequisite.value(),
                dependent.value()
            ),
        }
    }
}

impl Error for DependencyGraphError {}

/// A directed acyclic graph used to plan which parametric objects to regenerate.
///
/// An edge from `prerequisite` to `dependent` means that the dependent must be
/// regenerated after its prerequisite changes. The graph contains no evaluators
/// and does not mutate a CAD document.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DependencyGraph {
    dependents: BTreeMap<DependencyNode, BTreeSet<DependencyNode>>,
}

impl DependencyGraph {
    /// Adds a node. Adding an existing node is a no-op.
    pub fn add_node(&mut self, node: DependencyNode) {
        self.dependents.entry(node).or_default();
    }

    /// Removes a node and every edge to or from it. Returns whether it existed.
    pub fn remove_node(&mut self, node: DependencyNode) -> bool {
        if self.dependents.remove(&node).is_none() {
            return false;
        }
        for outgoing in self.dependents.values_mut() {
            outgoing.remove(&node);
        }
        true
    }

    /// Adds an edge from `prerequisite` to `dependent`.
    ///
    /// The operation is atomic: if either endpoint is unknown, the edge is a
    /// self-dependency, or it would create a cycle, the graph is unchanged.
    pub fn add_dependency(
        &mut self,
        prerequisite: DependencyNode,
        dependent: DependencyNode,
    ) -> Result<(), DependencyGraphError> {
        if !self.dependents.contains_key(&prerequisite) {
            return Err(DependencyGraphError::UnknownNode(prerequisite));
        }
        if !self.dependents.contains_key(&dependent) {
            return Err(DependencyGraphError::UnknownNode(dependent));
        }
        if prerequisite == dependent {
            return Err(DependencyGraphError::SelfDependency(prerequisite));
        }
        if self
            .dependents
            .get(&prerequisite)
            .is_some_and(|nodes| nodes.contains(&dependent))
        {
            return Ok(());
        }
        if self.reaches(dependent, prerequisite) {
            return Err(DependencyGraphError::Cycle {
                prerequisite,
                dependent,
            });
        }
        self.dependents
            .get_mut(&prerequisite)
            .expect("validated dependency node")
            .insert(dependent);
        Ok(())
    }

    /// Removes an edge. Returns whether the edge existed.
    pub fn remove_dependency(
        &mut self,
        prerequisite: DependencyNode,
        dependent: DependencyNode,
    ) -> bool {
        self.dependents
            .get_mut(&prerequisite)
            .is_some_and(|nodes| nodes.remove(&dependent))
    }

    /// Plans affected dependents in deterministic regeneration order.
    ///
    /// The changed nodes are validated and excluded from the returned plan.
    /// If multiple ready nodes can be regenerated, the lowest identifier comes
    /// first. Unknown changed nodes return an error without changing the graph.
    pub fn regeneration_order(
        &self,
        changed: impl IntoIterator<Item = DependencyNode>,
    ) -> Result<Vec<DependencyNode>, DependencyGraphError> {
        let changed: BTreeSet<_> = changed.into_iter().collect();
        for node in &changed {
            if !self.dependents.contains_key(node) {
                return Err(DependencyGraphError::UnknownNode(*node));
            }
        }

        let mut affected = BTreeSet::new();
        let mut pending: VecDeque<_> = changed.iter().copied().collect();
        while let Some(node) = pending.pop_front() {
            if let Some(dependents) = self.dependents.get(&node) {
                for dependent in dependents {
                    if !changed.contains(dependent) && affected.insert(*dependent) {
                        pending.push_back(*dependent);
                    }
                }
            }
        }

        let mut indegree: BTreeMap<_, usize> = affected.iter().map(|node| (*node, 0)).collect();
        for prerequisite in &affected {
            if let Some(dependents) = self.dependents.get(prerequisite) {
                for dependent in dependents.intersection(&affected) {
                    *indegree
                        .get_mut(dependent)
                        .expect("affected dependent has an indegree") += 1;
                }
            }
        }

        let mut ready: BTreeSet<_> = indegree
            .iter()
            .filter_map(|(node, degree)| (*degree == 0).then_some(*node))
            .collect();
        let mut order = Vec::with_capacity(affected.len());
        while let Some(node) = ready.pop_first() {
            order.push(node);
            if let Some(dependents) = self.dependents.get(&node) {
                for dependent in dependents.intersection(&affected) {
                    let degree = indegree
                        .get_mut(dependent)
                        .expect("affected dependent has an indegree");
                    *degree -= 1;
                    if *degree == 0 {
                        ready.insert(*dependent);
                    }
                }
            }
        }

        debug_assert_eq!(order.len(), affected.len());
        Ok(order)
    }

    /// Whether the graph contains a node.
    pub fn contains_node(&self, node: DependencyNode) -> bool {
        self.dependents.contains_key(&node)
    }

    /// Number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.dependents.len()
    }

    fn reaches(&self, start: DependencyNode, target: DependencyNode) -> bool {
        let mut seen = BTreeSet::new();
        let mut pending = vec![start];
        while let Some(node) = pending.pop() {
            if node == target {
                return true;
            }
            if seen.insert(node)
                && let Some(dependents) = self.dependents.get(&node)
            {
                pending.extend(dependents.iter().copied());
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{DependencyGraph, DependencyGraphError, DependencyNode};

    fn node(value: u64) -> DependencyNode {
        DependencyNode::new(value)
    }

    #[test]
    fn plans_affected_nodes_in_stable_topological_order() {
        let mut graph = DependencyGraph::default();
        for value in 1..=5 {
            graph.add_node(node(value));
        }
        graph.add_dependency(node(1), node(3)).unwrap();
        graph.add_dependency(node(1), node(2)).unwrap();
        graph.add_dependency(node(2), node(4)).unwrap();
        graph.add_dependency(node(3), node(4)).unwrap();
        graph.add_dependency(node(4), node(5)).unwrap();

        assert_eq!(
            graph.regeneration_order([node(1)]).unwrap(),
            [node(2), node(3), node(4), node(5)]
        );
        assert_eq!(
            graph.regeneration_order([node(2)]).unwrap(),
            [node(4), node(5)]
        );
        assert!(graph.regeneration_order([]).unwrap().is_empty());
    }

    #[test]
    fn rejects_cycles_without_changing_the_graph() {
        let mut graph = DependencyGraph::default();
        for value in 1..=3 {
            graph.add_node(node(value));
        }
        graph.add_dependency(node(1), node(2)).unwrap();
        graph.add_dependency(node(2), node(3)).unwrap();
        let before = graph.clone();

        assert_eq!(
            graph.add_dependency(node(3), node(1)),
            Err(DependencyGraphError::Cycle {
                prerequisite: node(3),
                dependent: node(1),
            })
        );
        assert_eq!(graph, before);
    }

    #[test]
    fn rejects_unknown_and_self_dependencies_without_mutation() {
        let mut graph = DependencyGraph::default();
        graph.add_node(node(1));
        let before = graph.clone();

        assert_eq!(
            graph.add_dependency(node(1), node(2)),
            Err(DependencyGraphError::UnknownNode(node(2)))
        );
        assert_eq!(
            graph.add_dependency(node(1), node(1)),
            Err(DependencyGraphError::SelfDependency(node(1)))
        );
        assert_eq!(
            graph.regeneration_order([node(2)]),
            Err(DependencyGraphError::UnknownNode(node(2)))
        );
        assert_eq!(graph, before);
    }

    #[test]
    fn removing_a_node_removes_its_incoming_and_outgoing_edges() {
        let mut graph = DependencyGraph::default();
        for value in 1..=3 {
            graph.add_node(node(value));
        }
        graph.add_dependency(node(1), node(2)).unwrap();
        graph.add_dependency(node(2), node(3)).unwrap();

        assert!(graph.remove_node(node(2)));
        assert!(!graph.contains_node(node(2)));
        assert_eq!(graph.regeneration_order([node(1)]).unwrap(), []);
        assert!(!graph.remove_node(node(2)));
    }
}

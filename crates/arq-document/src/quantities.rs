//! Quantities for the budget (ADR 0028 D3, D5): how many times each block
//! is placed in model space, a nested reference counting once for each
//! placement of the block holding it, down any number of levels.

use std::collections::{HashMap, VecDeque};

use acadrust::tables::TableEntry;
use acadrust::{EntityType, Handle};

use crate::document::Document;
use crate::entity::EntityId;
use crate::error::{DocumentError, DocumentResult};
use crate::translate::handle;

/// One reference as the count sees it: the block record it draws, the
/// block it counts for (a dynamic block's, through its anonymous copy),
/// and how many copies it places (a MINSERT's rows times columns).
#[derive(Clone, Copy)]
struct Placement {
    draws: Handle,
    counts_for: Option<Handle>,
    copies: u64,
}

impl Document {
    /// How many times each block is placed in model space (ADR 0028 D3):
    /// its references there, and those inside other blocks once for each
    /// placement of the block holding them. A dynamic block's anonymous
    /// copies count for it; a MINSERT counts each copy; references on off
    /// or frozen layers count; those only in layouts do not. Only blocks
    /// INSERT can place are listed, by the block table's spelling; blocks
    /// placed nowhere in model space are left out.
    pub fn block_quantities(&self) -> HashMap<String, u64> {
        let model = self
            .cad()
            .block_records
            .iter()
            .find(|b| b.is_model_space())
            .map(|b| b.entity_handles.clone())
            .unwrap_or_default();
        self.quantities(&model)
    }

    /// As `block_quantities`, counting only references `ids` and what they
    /// hold (ADR 0028 D5, Selection only). Ids that are not references are
    /// ignored, each id counts once; an id the drawing does not hold is an
    /// error.
    pub fn block_quantities_of(&self, ids: &[EntityId]) -> DocumentResult<HashMap<String, u64>> {
        let mut chosen: Vec<Handle> = Vec::new();
        for &id in ids {
            if self.cad().get_entity(handle(id)).is_none() {
                return Err(DocumentError::EntityNotFound(id));
            }
            if !chosen.contains(&handle(id)) {
                chosen.push(handle(id));
            }
        }
        Ok(self.quantities(&chosen))
    }

    /// The quantities of the references among `placed` and of everything
    /// they hold. Blocks are visited in an order where a block comes after
    /// every block holding it; one that holds itself through others (a
    /// broken file: ADR 0023 refuses it) is counted where it is placed, but
    /// what it holds is not.
    fn quantities(&self, placed: &[Handle]) -> HashMap<String, u64> {
        let cad = self.cad();
        let copies = self.anonymous_copies();
        let placement = |h: Handle| -> Option<Placement> {
            let EntityType::Insert(insert) = cad.get_entity(h)? else {
                return None;
            };
            Some(Placement {
                draws: cad.block_records.get(&insert.block_name)?.handle(),
                counts_for: self.definition_of(h, &copies),
                copies: u64::from(insert.row_count.max(1)) * u64::from(insert.column_count.max(1)),
            })
        };
        // What each block holds, and how many references hold it.
        let mut inside: HashMap<Handle, Vec<Placement>> = HashMap::new();
        let mut holders: HashMap<Handle, usize> = HashMap::new();
        for record in cad.block_records.iter() {
            let held: Vec<Placement> = record
                .entity_handles
                .iter()
                .filter_map(|&h| placement(h))
                .collect();
            for p in &held {
                *holders.entry(p.draws).or_insert(0) += 1;
            }
            inside.insert(record.handle(), held);
        }
        let mut counts: HashMap<Handle, u64> = HashMap::new();
        // How many times each block is placed, so far.
        let mut times: HashMap<Handle, u64> = HashMap::new();
        let mut add = |p: &Placement, n: u64, times: &mut HashMap<Handle, u64>| {
            let n = n.saturating_mul(p.copies);
            if let Some(block) = p.counts_for {
                let count = counts.entry(block).or_insert(0);
                *count = count.saturating_add(n);
            }
            let t = times.entry(p.draws).or_insert(0);
            *t = t.saturating_add(n);
        };
        for p in placed.iter().filter_map(|&h| placement(h)) {
            add(&p, 1, &mut times);
        }
        // Blocks no reference holds first, then each once all its holders
        // are done (Kahn's order).
        let mut ready: VecDeque<Handle> = inside
            .keys()
            .filter(|b| !holders.contains_key(b))
            .copied()
            .collect();
        while let Some(block) = ready.pop_front() {
            let n = times.get(&block).copied().unwrap_or(0);
            for p in inside.get(&block).into_iter().flatten() {
                if n > 0 {
                    add(p, n, &mut times);
                }
                if let Some(left) = holders.get_mut(&p.draws) {
                    *left -= 1;
                    if *left == 0 {
                        ready.push_back(p.draws);
                    }
                }
            }
        }
        let placeable: HashMap<Handle, String> = self
            .block_names()
            .into_iter()
            .filter_map(|n| cad.block_records.get(&n).map(|r| (r.handle(), n.clone())))
            .collect();
        counts
            .into_iter()
            .filter_map(|(h, n)| placeable.get(&h).map(|name| (name.clone(), n)))
            .collect()
    }
}

#[cfg(test)]
#[path = "quantities_tests.rs"]
mod tests;

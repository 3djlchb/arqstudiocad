//! `CadDocument`: one open drawing with its transactions and undo history
//! (contract §6, ADR 0011 D3.5 and D3.9).
//!
//! Every operation runs in [`CadDocument::transact`]: it is recorded as one
//! change and becomes one undo step, or is rolled back completely when it
//! fails. A group ([`CadDocument::begin_group`]) gathers the operations of a
//! command into a single undo step; a failure inside it rolls back the whole
//! group.

use std::path::Path;

use arq_document::{Change, ChangeSet, Document};

use crate::error::{CadError, CadResult};

/// One undo step: the changes it made, in order.
struct Step {
    name: String,
    changes: Vec<Change>,
    summary: ChangeSet,
}

/// The step being built by an open group.
struct Group {
    step: Step,
    /// How many `begin_group` calls are nested inside the outermost one.
    depth: usize,
}

/// One open drawing: the document plus its history.
pub struct CadDocument {
    doc: Document,
    undo: Vec<Step>,
    redo: Vec<Step>,
    group: Option<Group>,
}

impl CadDocument {
    /// A new, empty drawing.
    pub fn new() -> Self {
        Self::from_document(Document::new())
    }

    /// Opens a DWG or DXF file.
    pub fn open(path: impl AsRef<Path>) -> CadResult<Self> {
        Ok(Self::from_document(Document::open(path)?))
    }

    /// Wraps a loaded document, with an empty history.
    pub fn from_document(doc: Document) -> Self {
        Self {
            doc,
            undo: Vec::new(),
            redo: Vec::new(),
            group: None,
        }
    }

    /// Saves as DWG or DXF, by the file extension. The history is kept.
    pub fn save(&self, path: impl AsRef<Path>) -> CadResult<()> {
        Ok(self.doc.save(path)?)
    }

    /// Read access to the drawing. Changes go through this type's operations.
    pub fn document(&self) -> &Document {
        &self.doc
    }

    /// Runs `f` as one transaction named `name` (the undo step's name, an
    /// English command name such as `MOVE`).
    ///
    /// On success the change is committed: outside a group it becomes one
    /// undo step and clears redo; inside a group it joins the group's step.
    /// Returns `f`'s value and what this change did. On failure everything
    /// `f` did is rolled back and, inside a group, the whole group too.
    pub(crate) fn transact<T>(
        &mut self,
        name: &str,
        f: impl FnOnce(&mut Document) -> CadResult<T>,
    ) -> CadResult<(T, ChangeSet)> {
        self.doc.begin_change()?;
        let value = match f(&mut self.doc) {
            Ok(value) => value,
            Err(e) => {
                self.doc.rollback_change()?;
                if self.group.is_some() {
                    self.abort_group()?;
                }
                return Err(e);
            }
        };
        let change = self.doc.commit_change()?;
        let summary = change.summary().clone();
        if summary.is_empty() {
            return Ok((value, summary));
        }
        match self.group.as_mut() {
            Some(group) => {
                group.step.summary = group.step.summary.then(&summary);
                group.step.changes.push(change);
            }
            None => {
                self.redo.clear();
                self.undo.push(Step {
                    name: name.to_owned(),
                    changes: vec![change],
                    summary: summary.clone(),
                });
            }
        }
        Ok((value, summary))
    }

    /// Starts a group: the operations until the matching
    /// [`end_group`](Self::end_group) are undone as one step named `name`.
    /// Groups nest; only the outermost one makes a step, with its name.
    pub fn begin_group(&mut self, name: &str) {
        match self.group.as_mut() {
            Some(group) => group.depth += 1,
            None => {
                self.group = Some(Group {
                    step: Step {
                        name: name.to_owned(),
                        changes: Vec::new(),
                        summary: ChangeSet::default(),
                    },
                    depth: 0,
                });
            }
        }
    }

    /// Ends the innermost group. Ending the outermost one records its step
    /// (if it changed anything) and returns what the whole group did; ending
    /// a nested one returns an empty change set.
    pub fn end_group(&mut self) -> CadResult<ChangeSet> {
        let group = self.group.as_mut().ok_or_else(no_group)?;
        if group.depth > 0 {
            group.depth -= 1;
            return Ok(ChangeSet::default());
        }
        let step = self.group.take().map(|g| g.step).ok_or_else(no_group)?;
        let summary = step.summary.clone();
        if !step.changes.is_empty() {
            self.redo.clear();
            self.undo.push(step);
        }
        Ok(summary)
    }

    /// Cancels the open group at every nesting level, putting the drawing
    /// back as it was before it began; returns what that did.
    pub fn cancel_group(&mut self) -> CadResult<ChangeSet> {
        if self.group.is_none() {
            return Err(no_group());
        }
        self.abort_group()
    }

    /// Whether a group is open.
    pub fn in_group(&self) -> bool {
        self.group.is_some()
    }

    fn abort_group(&mut self) -> CadResult<ChangeSet> {
        let Some(mut group) = self.group.take() else {
            return Ok(ChangeSet::default());
        };
        for change in group.step.changes.iter_mut().rev() {
            self.doc.undo_change(change)?;
        }
        Ok(group.step.summary.inverted())
    }

    /// Undoes the last step; returns what that did.
    pub fn undo(&mut self) -> CadResult<ChangeSet> {
        self.check_no_group()?;
        let mut step = self
            .undo
            .pop()
            .ok_or_else(|| CadError::InvalidOperation("nothing to undo".to_owned()))?;
        for change in step.changes.iter_mut().rev() {
            self.doc.undo_change(change)?;
        }
        let summary = step.summary.inverted();
        self.redo.push(step);
        Ok(summary)
    }

    /// Redoes the last undone step; returns what that did.
    pub fn redo(&mut self) -> CadResult<ChangeSet> {
        self.check_no_group()?;
        let mut step = self
            .redo
            .pop()
            .ok_or_else(|| CadError::InvalidOperation("nothing to redo".to_owned()))?;
        for change in &mut step.changes {
            self.doc.redo_change(change)?;
        }
        let summary = step.summary.clone();
        self.undo.push(step);
        Ok(summary)
    }

    /// Whether there is a step to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there is a step to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Name of the step [`undo`](Self::undo) would revert.
    pub fn undo_name(&self) -> Option<&str> {
        self.undo.last().map(|s| s.name.as_str())
    }

    /// Name of the step [`redo`](Self::redo) would apply.
    pub fn redo_name(&self) -> Option<&str> {
        self.redo.last().map(|s| s.name.as_str())
    }

    fn check_no_group(&self) -> CadResult<()> {
        if self.group.is_some() {
            return Err(CadError::InvalidOperation(
                "cannot undo or redo while a group is open".to_owned(),
            ));
        }
        Ok(())
    }
}

impl Default for CadDocument {
    fn default() -> Self {
        Self::new()
    }
}

fn no_group() -> CadError {
    CadError::InvalidOperation("no group is open".to_owned())
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;

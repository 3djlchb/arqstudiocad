//! Purging blocks nothing uses (PURGE, ADR 0025 step 8e): their contents,
//! the objects they own and their records go, in one change; the blocks
//! they held that nothing else uses can go with them, as AutoCAD's "Purge
//! nested items". Dynamic blocks wait for the parametric engine (04.1).

use std::collections::{BTreeSet, HashSet};

use acadrust::tables::TableEntry;
use acadrust::{EntityType, Handle};

use crate::document::Document;
use crate::error::{DocumentError, DocumentResult};

impl Document {
    /// The blocks INSERT can place that nothing references, anywhere, nor
    /// through an anonymous copy (as `block_references` counts), sorted as
    /// `block_names`. Dynamic ones are listed; `purge_blocks` refuses them.
    pub fn unused_blocks(&self) -> Vec<String> {
        let used = self.block_references();
        self.block_names()
            .into_iter()
            .filter(|n| !used.contains_key(n))
            .collect()
    }

    /// Purges blocks `names`, found ignoring case; each must be unused and
    /// not dynamic. With `nested`, the blocks they held go too once nothing
    /// else uses them, and so on down. Returns the blocks purged, as the
    /// drawing spelled them, in the order purged.
    pub fn purge_blocks(&mut self, names: &[String], nested: bool) -> DocumentResult<Vec<String>> {
        let unused = self.unused_blocks();
        let mut chosen: Vec<String> = Vec::new();
        for name in names {
            let Some(block) = unused.iter().find(|n| n.eq_ignore_ascii_case(name)) else {
                let known = self
                    .block_names()
                    .iter()
                    .any(|n| n.eq_ignore_ascii_case(name));
                return Err(DocumentError::InvalidBlockName(if known {
                    format!("{name} is in use")
                } else {
                    format!("no block named {name:?}")
                }));
            };
            if self.is_dynamic_block(block) {
                return Err(DocumentError::InvalidBlockName(format!(
                    "{block} is a dynamic block"
                )));
            }
            if !chosen.contains(block) {
                chosen.push(block.clone());
            }
        }
        self.touch_structure();
        let mut purged = Vec::new();
        let mut held: BTreeSet<String> = BTreeSet::new();
        for block in chosen {
            held.extend(self.remove_block(&block)?);
            purged.push(block);
        }
        if !nested {
            return Ok(purged);
        }
        // Those held by what went, now unused and not dynamic, until none.
        loop {
            let next: Vec<String> = self
                .unused_blocks()
                .into_iter()
                .filter(|n| held.iter().any(|h| h.eq_ignore_ascii_case(n)))
                .filter(|n| !self.is_dynamic_block(n))
                .collect();
            if next.is_empty() {
                return Ok(purged);
            }
            for block in next {
                held.extend(self.remove_block(&block)?);
                purged.push(block);
            }
        }
    }

    /// Removes block `block`: its contents, its BLOCK and ENDBLK, every
    /// object owned by any of them or by its record (extension
    /// dictionaries and what they hold), and its record. Returns the blocks
    /// its contents referenced. The caller has touched the structure.
    fn remove_block(&mut self, block: &str) -> DocumentResult<BTreeSet<String>> {
        let cad = self.cad();
        let record = cad
            .block_records
            .get(block)
            .ok_or_else(|| DocumentError::InvalidBlockName(format!("no block named {block:?}")))?;
        // The BLOCK and ENDBLK, which the codec keeps as entities.
        let ends = [record.block_entity_handle, record.block_end_handle];
        let mut gone: HashSet<Handle> = HashSet::from([record.handle()]);
        gone.extend(ends);
        let mut held = BTreeSet::new();
        for e in cad.entities_in_block(block) {
            gone.insert(e.as_entity().handle());
            if let EntityType::Insert(insert) = e {
                gone.extend(insert.attributes.iter().map(|a| a.common.handle));
                held.insert(insert.block_name.clone());
            }
        }
        // What those own, and what that owns, and so on.
        let mut owned: Vec<Handle> = Vec::new();
        loop {
            let more: Vec<Handle> = cad
                .objects
                .keys()
                .copied()
                .filter(|h| !gone.contains(h))
                .filter(|&h| cad.object_owner(h).is_some_and(|o| gone.contains(&o)))
                .collect();
            if more.is_empty() {
                break;
            }
            gone.extend(more.iter().copied());
            owned.extend(more);
        }
        let record = self.clear_block(block)?;
        let cad = self.cad_mut();
        for h in ends {
            if cad.get_entity(h).is_some() {
                cad.remove_entity(h);
            }
        }
        for h in owned {
            cad.objects.remove(&h);
        }
        cad.object_xdata.remove(&record);
        cad.block_records.remove(block);
        Ok(held)
    }
}

#[cfg(test)]
#[path = "purge_tests.rs"]
mod tests;

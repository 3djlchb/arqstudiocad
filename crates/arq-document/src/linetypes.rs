//! Simple AutoCAD `.lin` definitions and loaded drawing linetypes.

use crate::change::LayerNote;
use acadrust::tables::linetype::{LineType, LineTypeElement};
use acadrust::tables::normalize_name;

/// A simple linetype definition read from a `.lin` file.
#[derive(Clone, Debug, PartialEq)]
pub struct LinetypeDefinition {
    /// Symbol-table name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Dash, gap and dot lengths; positive is dash, negative gap, zero dot.
    pub pattern: Vec<f64>,
}

/// Parses simple A-aligned `.lin` definitions. Complex text and shape records
/// are rejected explicitly instead of being silently reduced to a wrong line.
pub fn parse_linetype_file(contents: &str) -> Result<Vec<LinetypeDefinition>, String> {
    let contents = contents.trim_start_matches('\u{feff}');
    let mut result = Vec::new();
    let mut header: Option<(String, String)> = None;
    for (line_number, raw) in contents.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if let Some(header_line) = line.strip_prefix('*') {
            if let Some((name, _description)) = header.take() {
                return Err(format!("line {}: missing pattern for {name}", line_number));
            }
            let (name, description) = header_line.split_once(',').unwrap_or((header_line, ""));
            let name = name.trim();
            if name.is_empty() || name.chars().any(char::is_control) {
                return Err(format!("line {}: invalid linetype name", line_number + 1));
            }
            header = Some((name.to_owned(), description.trim().to_owned()));
            continue;
        }
        let Some((name, description)) = header.take() else {
            return Err(format!(
                "line {}: pattern without a header",
                line_number + 1
            ));
        };
        if line.contains('[') || line.contains(']') {
            return Err(format!(
                "line {}: complex text/shape linetypes are unsupported",
                line_number + 1
            ));
        }
        let (alignment, values) = line.split_once(',').unwrap_or(("A", ""));
        if !alignment.trim().eq_ignore_ascii_case("A") {
            return Err(format!(
                "line {}: only A alignment is supported",
                line_number + 1
            ));
        }
        let pattern = values
            .split(',')
            .map(|value| {
                value
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| format!("line {}: invalid pattern value", line_number + 1))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if pattern.is_empty()
            || pattern.len() > 100
            || pattern.iter().any(|v| !v.is_finite())
            || pattern.iter().all(|v| *v == 0.0)
        {
            return Err(format!(
                "line {}: pattern must contain finite dash, gap or dot values",
                line_number + 1
            ));
        }
        result.push(LinetypeDefinition {
            name,
            description,
            pattern,
        });
    }
    if let Some((name, _)) = header {
        return Err(format!("missing pattern for {name}"));
    }
    if result.is_empty() {
        return Err("the file contains no supported linetypes".to_owned());
    }
    let mut names = std::collections::BTreeSet::new();
    if result
        .iter()
        .any(|item| !names.insert(normalize_name(&item.name)))
    {
        return Err("the file contains duplicate linetype names".to_owned());
    }
    Ok(result)
}

/// Builds a codec table entry for a simple linetype definition.
pub(crate) fn codec_linetype(definition: &LinetypeDefinition) -> LineType {
    let mut linetype = LineType::new(&definition.name);
    linetype.description.clone_from(&definition.description);
    linetype.elements = definition
        .pattern
        .iter()
        .map(|value| LineTypeElement {
            length: *value,
            complex: None,
        })
        .collect();
    linetype.pattern_length = definition.pattern.iter().map(|value| value.abs()).sum();
    linetype
}

impl crate::Document {
    /// Parses simple A-aligned `.lin` definitions.
    pub fn parse_linetype_file(contents: &str) -> Result<Vec<LinetypeDefinition>, String> {
        parse_linetype_file(contents)
    }

    /// Lists simple definitions loaded in the drawing.
    pub fn linetype_definitions(&self) -> Vec<LinetypeDefinition> {
        self.cad()
            .line_types
            .iter()
            .filter_map(|entry| {
                if entry.name.eq_ignore_ascii_case("ByLayer")
                    || entry.name.eq_ignore_ascii_case("ByBlock")
                {
                    return None;
                }
                if entry
                    .elements
                    .iter()
                    .any(|element| element.complex.is_some())
                {
                    return None;
                }
                Some(LinetypeDefinition {
                    name: entry.name.clone(),
                    description: entry.description.clone(),
                    pattern: entry
                        .elements
                        .iter()
                        .map(|element| element.length)
                        .collect(),
                })
            })
            .collect()
    }

    /// Creates or updates a simple linetype. Standard definitions cannot be edited.
    pub fn set_linetype(
        &mut self,
        definition: &LinetypeDefinition,
    ) -> Result<(), crate::DocumentError> {
        validate_definition(definition)?;
        let standard = ["Continuous", "ByLayer", "ByBlock"];
        if standard
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&definition.name))
        {
            return Err(crate::DocumentError::UnsupportedOperation(
                "standard linetypes cannot be edited".to_owned(),
            ));
        }
        if self
            .cad()
            .line_types
            .get(&definition.name)
            .is_some_and(|existing| existing.elements.iter().any(|item| item.complex.is_some()))
        {
            return Err(crate::DocumentError::UnsupportedOperation(
                "complex text/shape linetypes cannot be replaced by a simple definition".to_owned(),
            ));
        }
        self.touch_structure();
        let mut entry = codec_linetype(definition);
        if let Some(existing) = self.cad().line_types.get(&definition.name) {
            entry.handle = existing.handle;
            self.cad_mut().line_types.add_or_replace(entry);
        } else {
            entry.handle = self.cad_mut().allocate_handle();
            self.cad_mut()
                .line_types
                .add(entry)
                .map_err(|error| crate::DocumentError::Codec(error.to_string()))?;
        }
        self.note_document();
        Ok(())
    }

    /// Renames a linetype and updates layer and entity references.
    pub fn rename_linetype(
        &mut self,
        old_name: &str,
        new_name: &str,
    ) -> Result<(), crate::DocumentError> {
        validate_definition(&LinetypeDefinition {
            name: new_name.to_owned(),
            description: String::new(),
            pattern: vec![1.0, -1.0],
        })?;
        if ["Continuous", "ByLayer", "ByBlock"]
            .iter()
            .any(|name| name.eq_ignore_ascii_case(old_name))
        {
            return Err(crate::DocumentError::UnsupportedOperation(
                "standard linetypes cannot be renamed".to_owned(),
            ));
        }
        let exists = self.cad().line_types.contains(old_name);
        if !exists {
            return Err(crate::DocumentError::UnsupportedOperation(format!(
                "linetype {old_name:?} is not loaded"
            )));
        }
        if self.cad().line_types.contains(new_name) && !old_name.eq_ignore_ascii_case(new_name) {
            return Err(crate::DocumentError::UnsupportedOperation(format!(
                "linetype {new_name:?} already exists"
            )));
        }
        self.touch_structure();
        self.record_all_entities();
        let mut changed_layers = Vec::new();
        for layer in self.cad_mut().layers.iter_mut() {
            if layer.line_type.eq_ignore_ascii_case(old_name) {
                layer.line_type = new_name.to_owned();
                changed_layers.push(layer.handle.value());
            }
        }
        for entity in self.cad_mut().entities_mut() {
            let common = entity.common_mut();
            if common.linetype.eq_ignore_ascii_case(old_name) {
                common.linetype = new_name.to_owned();
            }
        }
        self.cad_mut()
            .line_types
            .rename(old_name, new_name)
            .map_err(|error| crate::DocumentError::Codec(error.to_string()))?;
        self.sync_layers();
        for id in changed_layers {
            self.note_layer(id, LayerNote::Modified);
        }
        self.note_document();
        Ok(())
    }

    /// Deletes an unused custom linetype from the drawing.
    pub fn delete_linetype(&mut self, name: &str) -> Result<(), crate::DocumentError> {
        if ["Continuous", "ByLayer", "ByBlock"]
            .iter()
            .any(|standard| standard.eq_ignore_ascii_case(name))
        {
            return Err(crate::DocumentError::UnsupportedOperation(
                "standard linetypes cannot be deleted".to_owned(),
            ));
        }
        let in_use_by_layer = self
            .cad()
            .layers
            .iter()
            .any(|layer| layer.line_type.eq_ignore_ascii_case(name));
        let in_use_by_entity = self
            .cad()
            .entities()
            .any(|entity| entity.common().linetype.eq_ignore_ascii_case(name));
        if in_use_by_layer || in_use_by_entity {
            return Err(crate::DocumentError::UnsupportedOperation(format!(
                "linetype {name:?} is in use"
            )));
        }
        if !self.cad().line_types.contains(name) {
            return Err(crate::DocumentError::UnsupportedOperation(format!(
                "linetype {name:?} is not loaded"
            )));
        }
        self.touch_structure();
        self.cad_mut().line_types.remove(name);
        self.note_document();
        Ok(())
    }

    /// Loads simple definitions that are not already present in the drawing.
    /// Existing definitions are preserved; callers can reload explicitly.
    pub fn load_linetypes(
        &mut self,
        definitions: &[LinetypeDefinition],
    ) -> Result<(), crate::DocumentError> {
        if definitions.is_empty() {
            return Err(crate::DocumentError::UnsupportedOperation(
                "no linetypes were selected".to_owned(),
            ));
        }
        let mut names = std::collections::BTreeSet::new();
        for definition in definitions {
            validate_definition(definition)?;
            let normalized = normalize_name(&definition.name);
            if !names.insert(normalized.clone()) || self.cad().line_types.contains(&definition.name)
            {
                return Err(crate::DocumentError::UnsupportedOperation(format!(
                    "linetype {:?} already exists or is selected twice",
                    definition.name
                )));
            }
        }
        self.touch_structure();
        for definition in definitions {
            let mut entry = codec_linetype(definition);
            entry.handle = self.cad_mut().allocate_handle();
            self.cad_mut()
                .line_types
                .add(entry)
                .map_err(|error| crate::DocumentError::Codec(error.to_string()))?;
        }
        self.note_document();
        Ok(())
    }
}

fn validate_definition(definition: &LinetypeDefinition) -> Result<(), crate::DocumentError> {
    let invalid_name = definition.name.is_empty()
        || definition.name.trim() != definition.name
        || definition.name.chars().count() > 255
        || definition.name.chars().any(|ch| {
            ch.is_control()
                || matches!(
                    ch,
                    '<' | '>' | '/' | '\\' | '"' | ':' | ';' | '?' | '*' | '|' | ',' | '=' | '`'
                )
        });
    let invalid_pattern = definition.pattern.is_empty()
        || definition.pattern.len() > 100
        || definition.pattern.iter().any(|value| !value.is_finite())
        || definition.pattern.iter().all(|value| *value == 0.0);
    if invalid_name
        || invalid_pattern
        || definition.name.eq_ignore_ascii_case("ByLayer")
        || definition.name.eq_ignore_ascii_case("ByBlock")
    {
        return Err(crate::DocumentError::UnsupportedOperation(format!(
            "invalid simple linetype definition {:?}",
            definition.name
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "linetypes_tests.rs"]
mod tests;

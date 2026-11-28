//! AutoCAD `.lin` definitions and loaded drawing linetypes.

use crate::change::LayerNote;
use acadrust::tables::linetype::{
    LineType, LineTypeComplexContent as CodecLinetypeComplexContent, LineTypeComplexData,
    LineTypeElement,
};
use acadrust::tables::{TextStyle, normalize_name};

/// A simple linetype definition read from a `.lin` file.
#[derive(Clone, Debug, PartialEq)]
pub struct LinetypeDefinition {
    /// Symbol-table name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Dash, gap and dot lengths; positive is dash, negative gap, zero dot.
    pub pattern: Vec<f64>,
    /// Embedded text and shape descriptors attached to pattern elements.
    pub complex: Vec<LinetypeComplexDefinition>,
}

/// Text or shape drawn at one pattern element.
#[derive(Clone, Debug, PartialEq)]
pub struct LinetypeComplexDefinition {
    /// Index in `LinetypeDefinition::pattern`.
    pub segment_index: usize,
    /// The embedded content and its text style or SHX file.
    pub content: LinetypeComplexContent,
    /// Scale multiplier from `S=`.
    pub scale: f64,
    /// Rotation in radians.
    pub rotation: f64,
    /// Rotation relative to the path, absolute, or kept upright.
    pub rotation_mode: LinetypeRotationMode,
    /// Offset along and perpendicular to the path.
    pub offset: [f64; 2],
}

/// Content referenced by a complex `.lin` descriptor.
#[derive(Clone, Debug, PartialEq)]
pub enum LinetypeComplexContent {
    /// Literal text drawn with the named text style.
    Text {
        /// Text content, which may include commas when quoted in the LIN file.
        text: String,
        /// Text style name from the drawing's STYLE table.
        style_name: String,
    },
    /// Named glyph from the referenced SHX shape file.
    Shape {
        /// Glyph name in the SHX file.
        name: String,
        /// SHX file name or path.
        shape_file: String,
    },
}

/// Complex content orientation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LinetypeRotationMode {
    /// Rotation follows the path tangent.
    #[default]
    Relative,
    /// Rotation is relative to the world axes.
    Absolute,
    /// Rotation follows the path and is flipped to remain readable.
    Upright,
}

/// Parses A-aligned simple and complex `.lin` definitions.
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
        let (alignment, values) = line.split_once(',').unwrap_or(("A", ""));
        if !alignment.trim().eq_ignore_ascii_case("A") {
            return Err(format!(
                "line {}: only A alignment is supported",
                line_number + 1
            ));
        }
        let tokens = split_pattern_tokens(values)
            .map_err(|message| format!("line {}: {message}", line_number + 1))?;
        let mut pattern = Vec::new();
        let mut complex = Vec::new();
        for token in tokens {
            if token.starts_with('[') {
                let segment_index = pattern.len().checked_sub(1).ok_or_else(|| {
                    format!(
                        "line {}: a complex element needs a preceding segment",
                        line_number + 1
                    )
                })?;
                if complex
                    .iter()
                    .any(|item: &LinetypeComplexDefinition| item.segment_index == segment_index)
                {
                    return Err(format!(
                        "line {}: multiple complex elements share one segment",
                        line_number + 1
                    ));
                }
                complex.push(
                    parse_complex_descriptor(&token)
                        .map_err(|message| format!("line {}: {message}", line_number + 1))?
                        .with_segment(segment_index),
                );
            } else {
                let value = token
                    .parse::<f64>()
                    .map_err(|_| format!("line {}: invalid pattern value", line_number + 1))?;
                pattern.push(value);
            }
        }
        if pattern.is_empty()
            || pattern.len() > 100
            || pattern.iter().any(|v| !v.is_finite())
            || (pattern.iter().all(|v| *v == 0.0) && complex.is_empty())
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
            complex,
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

impl LinetypeComplexDefinition {
    fn with_segment(mut self, segment_index: usize) -> Self {
        self.segment_index = segment_index;
        self
    }
}

fn split_pattern_tokens(values: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut start = 0;
    let (mut bracket_depth, mut quoted) = (0_u8, false);
    let mut escaped = false;
    for (index, ch) in values.char_indices() {
        if quoted && escaped {
            escaped = false;
            continue;
        }
        if quoted && ch == '\\' {
            escaped = true;
            continue;
        }
        match ch {
            '"' => quoted = !quoted,
            '[' if !quoted => bracket_depth = bracket_depth.saturating_add(1),
            ']' if !quoted => {
                if bracket_depth == 0 {
                    return Err("unmatched closing bracket".to_owned());
                }
                bracket_depth -= 1;
            }
            ',' if !quoted && bracket_depth == 0 => {
                tokens.push(values[start..index].trim().to_owned());
                start = index + 1;
            }
            _ => {}
        }
    }
    if quoted || bracket_depth != 0 {
        return Err("unterminated quoted text or complex descriptor".to_owned());
    }
    tokens.push(values[start..].trim().to_owned());
    if tokens.iter().any(String::is_empty) {
        return Err("empty pattern element".to_owned());
    }
    Ok(tokens)
}

fn parse_complex_descriptor(token: &str) -> Result<LinetypeComplexDefinition, String> {
    let inside = token
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| "invalid complex descriptor".to_owned())?;
    let fields = split_pattern_tokens(inside)?;
    if fields.len() < 2 {
        return Err("complex descriptor needs content and a style or SHX file".to_owned());
    }
    let is_text = fields[0].starts_with('"') && fields[0].ends_with('"');
    let first = unquote_field(&fields[0]);
    let second = unquote_field(&fields[1]);
    if first.is_empty() || second.is_empty() {
        return Err("complex descriptor content and reference cannot be empty".to_owned());
    }
    let content = if is_text {
        LinetypeComplexContent::Text {
            text: first,
            style_name: second,
        }
    } else {
        LinetypeComplexContent::Shape {
            name: first,
            shape_file: second,
        }
    };
    let (mut scale, mut rotation, mut rotation_mode, mut x, mut y) =
        (1.0, 0.0, LinetypeRotationMode::Relative, 0.0, 0.0);
    for field in fields.iter().skip(2) {
        let (key, value) = field
            .split_once('=')
            .ok_or_else(|| format!("invalid complex option {field:?}"))?;
        let number = value
            .trim()
            .parse::<f64>()
            .map_err(|_| format!("invalid complex option value {value:?}"))?;
        if !number.is_finite() {
            return Err("complex options must be finite".to_owned());
        }
        match key.trim().to_ascii_uppercase().as_str() {
            "S" => scale = number,
            "R" => {
                rotation = number.to_radians();
                rotation_mode = LinetypeRotationMode::Relative;
            }
            "A" => {
                rotation = number.to_radians();
                rotation_mode = LinetypeRotationMode::Absolute;
            }
            "U" => {
                rotation = number.to_radians();
                rotation_mode = LinetypeRotationMode::Upright;
            }
            "X" => x = number,
            "Y" => y = number,
            _ => return Err(format!("unknown complex option {key:?}")),
        }
    }
    if scale == 0.0 {
        return Err("complex element scale cannot be zero".to_owned());
    }
    Ok(LinetypeComplexDefinition {
        segment_index: 0,
        content,
        scale,
        rotation,
        rotation_mode,
        offset: [x, y],
    })
}

fn unquote_field(value: &str) -> String {
    let Some(value) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        return value.to_owned();
    };
    value.replace("\\\\", "\\").replace("\\\"", "\"")
}

/// Builds a codec table entry for a simple linetype definition.
pub(crate) fn codec_linetype(definition: &LinetypeDefinition) -> LineType {
    let mut linetype = LineType::new(&definition.name);
    linetype.description.clone_from(&definition.description);
    linetype.elements = definition
        .pattern
        .iter()
        .enumerate()
        .map(|(segment_index, value)| LineTypeElement {
            length: *value,
            complex: definition
                .complex
                .iter()
                .find(|complex| complex.segment_index == segment_index)
                .map(|complex| LineTypeComplexData {
                    content: match &complex.content {
                        LinetypeComplexContent::Text { text, .. } => {
                            CodecLinetypeComplexContent::Text { text: text.clone() }
                        }
                        LinetypeComplexContent::Shape { name, .. } => {
                            CodecLinetypeComplexContent::Shape {
                                shape_number: name
                                    .strip_prefix('#')
                                    .and_then(|number| number.parse().ok())
                                    .unwrap_or(0),
                            }
                        }
                    },
                    style_handle: acadrust::types::Handle::NULL,
                    scale: complex.scale,
                    rotation: complex.rotation,
                    absolute_rotation: complex.rotation_mode == LinetypeRotationMode::Absolute,
                    upright: complex.rotation_mode == LinetypeRotationMode::Upright,
                    source_flags: 0,
                    offset: complex.offset,
                }),
        })
        .collect();
    linetype.pattern_length = definition.pattern.iter().map(|value| value.abs()).sum();
    linetype
}

impl crate::Document {
    /// Parses simple and complex A-aligned `.lin` definitions.
    pub fn parse_linetype_file(contents: &str) -> Result<Vec<LinetypeDefinition>, String> {
        parse_linetype_file(contents)
    }

    /// Lists linetype definitions loaded in the drawing.
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
                let complex = entry
                    .elements
                    .iter()
                    .enumerate()
                    .filter_map(|(segment_index, element)| {
                        let data = element.complex.as_ref()?;
                        let style = self
                            .cad()
                            .text_styles
                            .iter()
                            .find(|style| style.handle == data.style_handle);
                        let content = match &data.content {
                            CodecLinetypeComplexContent::Text { text } => {
                                LinetypeComplexContent::Text {
                                    text: text.clone(),
                                    style_name: style.map_or_else(String::new, |s| s.name.clone()),
                                }
                            }
                            CodecLinetypeComplexContent::Shape { shape_number } => {
                                LinetypeComplexContent::Shape {
                                    name: format!("#{shape_number}"),
                                    shape_file: style
                                        .map_or_else(String::new, |s| s.font_file.clone()),
                                }
                            }
                        };
                        Some(LinetypeComplexDefinition {
                            segment_index,
                            content,
                            scale: data.scale,
                            rotation: data.rotation,
                            rotation_mode: if data.upright {
                                LinetypeRotationMode::Upright
                            } else if data.absolute_rotation {
                                LinetypeRotationMode::Absolute
                            } else {
                                LinetypeRotationMode::Relative
                            },
                            offset: data.offset,
                        })
                    })
                    .collect();
                Some(LinetypeDefinition {
                    name: entry.name.clone(),
                    description: entry.description.clone(),
                    pattern: entry
                        .elements
                        .iter()
                        .map(|element| element.length)
                        .collect(),
                    complex,
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
            && definition.complex.is_empty()
        {
            return Err(crate::DocumentError::UnsupportedOperation(
                "complex text/shape linetypes cannot be replaced by a simple definition".to_owned(),
            ));
        }
        self.touch_structure();
        self.ensure_linetype_styles(definition)?;
        let mut entry = self.codec_linetype_with_styles(definition)?;
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

    fn ensure_linetype_styles(
        &mut self,
        definition: &LinetypeDefinition,
    ) -> Result<(), crate::DocumentError> {
        for complex in &definition.complex {
            let (style_name, font_file, is_shape_file) = match &complex.content {
                LinetypeComplexContent::Text { style_name, .. } => {
                    (style_name.as_str(), None, false)
                }
                LinetypeComplexContent::Shape { shape_file, .. } => {
                    (shape_file.as_str(), Some(shape_file.as_str()), true)
                }
            };
            let exists = if is_shape_file {
                self.cad().text_styles.iter().any(|style| {
                    style.is_shape_file && style.font_file.eq_ignore_ascii_case(style_name)
                })
            } else {
                self.cad().text_styles.get(style_name).is_some()
            };
            if !exists {
                let mut style = TextStyle::new(style_name);
                if let Some(font_file) = font_file {
                    style.font_file = font_file.to_owned();
                    style.is_shape_file = true;
                }
                style.handle = self.cad_mut().allocate_handle();
                self.cad_mut()
                    .text_styles
                    .add(style)
                    .map_err(|error| crate::DocumentError::Codec(error.to_string()))?;
            }
        }
        Ok(())
    }

    fn codec_linetype_with_styles(
        &self,
        definition: &LinetypeDefinition,
    ) -> Result<LineType, crate::DocumentError> {
        let mut entry = codec_linetype(definition);
        for (index, element) in entry.elements.iter_mut().enumerate() {
            let Some(data) = element.complex.as_mut() else {
                continue;
            };
            let descriptor = definition
                .complex
                .iter()
                .find(|complex| complex.segment_index == index)
                .expect("a codec complex element comes from one LIN descriptor");
            let style = match &descriptor.content {
                LinetypeComplexContent::Text { style_name, .. } => {
                    self.cad().text_styles.get(style_name)
                }
                LinetypeComplexContent::Shape { shape_file, .. } => {
                    self.cad().text_styles.iter().find(|style| {
                        style.is_shape_file && style.font_file.eq_ignore_ascii_case(shape_file)
                    })
                }
            }
            .ok_or_else(|| {
                crate::DocumentError::UnsupportedOperation(
                    "the linetype text or shape style could not be resolved".to_owned(),
                )
            })?;
            data.style_handle = style.handle;
        }
        Ok(entry)
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
            complex: Vec::new(),
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

    /// Loads definitions that are not already present in the drawing.
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
            self.ensure_linetype_styles(definition)?;
            let mut entry = self.codec_linetype_with_styles(definition)?;
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
    let invalid_complex = definition.complex.iter().any(|item| {
        item.segment_index >= definition.pattern.len()
            || !item.scale.is_finite()
            || item.scale == 0.0
            || !item.rotation.is_finite()
            || item.offset.iter().any(|value| !value.is_finite())
            || match &item.content {
                LinetypeComplexContent::Text { text, style_name } => {
                    text.is_empty() || style_name.is_empty()
                }
                LinetypeComplexContent::Shape { name, shape_file } => {
                    name.is_empty() || shape_file.is_empty()
                }
            }
    });
    let invalid_pattern = definition.pattern.is_empty()
        || definition.pattern.len() > 100
        || definition.pattern.iter().any(|value| !value.is_finite())
        || (definition.pattern.iter().all(|value| *value == 0.0) && definition.complex.is_empty());
    if invalid_name
        || invalid_complex
        || invalid_pattern
        || definition.name.eq_ignore_ascii_case("ByLayer")
        || definition.name.eq_ignore_ascii_case("ByBlock")
    {
        return Err(crate::DocumentError::UnsupportedOperation(format!(
            "invalid linetype definition {:?}",
            definition.name
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "linetypes_tests.rs"]
mod tests;

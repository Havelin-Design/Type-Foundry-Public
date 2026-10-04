//! Pick a JSON import from the text of an opened file.
//!
//! A Type Foundry document stays `typefoundry.font`. A facetype / Three.js
//! typeface file and a webfontjson callback are read here and turned into that
//! document. The session still has one `open` command.

use serde_json::Value;

use crate::error::FoundryError;
use crate::font::{FONT_FORMAT, Font};

pub(crate) fn load_text(text: &str) -> Result<Font, FoundryError> {
    let text = text.trim_start_matches('\u{feff}').trim();
    let body = unwrap_jsonp(text);
    let value: Value =
        serde_json::from_str(body).map_err(|err| FoundryError::Json(err.to_string()))?;
    if value.as_array().is_some() {
        return Err(FoundryError::Import(
            "this is a webfontjson config, not a font. Import the generated file, the one whose JSON has a css field.".to_string(),
        ));
    }
    if value.get("format").and_then(Value::as_str) == Some(FONT_FORMAT) {
        return Font::from_json(text);
    }
    if crate::typeface::is_typeface(&value) {
        return crate::typeface::load_value(&value);
    }
    if crate::webfont::is_webfont_json(&value) {
        return crate::webfont::load_json_value(&value);
    }
    Font::from_json(text)
}

/// `callback({...});` is the webfontjson output. A bare JSON object is unchanged.
fn unwrap_jsonp(text: &str) -> &str {
    let trimmed = text.trim().trim_end_matches(';').trim();
    if trimmed.starts_with('{') {
        return trimmed;
    }
    let Some(open) = trimmed.find('(') else {
        return trimmed;
    };
    if !trimmed.ends_with(')') {
        return trimmed;
    }
    let name = &trimmed[..open];
    let name_ok = !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '$');
    if name_ok {
        trimmed[open + 1..trimmed.len() - 1].trim()
    } else {
        trimmed
    }
}

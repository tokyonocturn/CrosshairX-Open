//! Preset import/export.
//!
//! Two shapes are supported:
//!
//! * this project's own format (`{"format": "crosshairx-open/1", "layers": [...]}`)
//! * the community payload shape used by the original app
//!   (`{"crosshairId": [layer, ...], "name": "...", "timestamp": ...}`), including
//!   the `{"items": [...]}` page wrapper.
//!
//! Unknown fields are ignored, so files written by newer versions still load.

use serde_json::{json, Map, Value};

use crate::model::{Crosshair, Layer};

pub const FORMAT: &str = "crosshairx-open/1";

/// Serialise one preset in this project's format.
pub fn export_json(crosshair: &Crosshair) -> String {
    let value = json!({
        "format": FORMAT,
        "name": crosshair.name,
        "layers": crosshair.layers,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
}

/// Parse a preset document, returning every design it contains.
pub fn import_str(text: &str) -> Result<Vec<Crosshair>, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|e| format!("not valid JSON: {e}"))?;
    let mut out = Vec::new();
    collect(&value, &mut out)?;
    if out.is_empty() {
        return Err("no crosshairs found in file".to_string());
    }
    for crosshair in &mut out {
        crosshair.sanitize();
    }
    Ok(out)
}

fn collect(value: &Value, out: &mut Vec<Crosshair>) -> Result<(), String> {
    match value {
        Value::Array(items) => {
            for item in items {
                collect(item, out)?;
            }
            Ok(())
        }
        Value::Object(map) => {
            if let Some(items) = map.get("items").and_then(|v| v.as_array()) {
                for item in items {
                    collect(item, out)?;
                }
                return Ok(());
            }
            if let Some(crosshair) = from_object(map)? {
                out.push(crosshair);
            }
            Ok(())
        }
        _ => Err("expected a JSON object or array".to_string()),
    }
}

fn from_object(map: &Map<String, Value>) -> Result<Option<Crosshair>, String> {
    // Native format.
    if map.contains_key("layers") {
        let crosshair: Crosshair = serde_json::from_value(Value::Object(map.clone()))
            .map_err(|e| format!("invalid preset: {e}"))?;
        return Ok(Some(crosshair));
    }
    // Community format.
    if let Some(layers) = map.get("crosshairId") {
        let layers: Vec<Layer> = serde_json::from_value(layers.clone())
            .map_err(|e| format!("invalid crosshair layers: {e}"))?;
        let name = map
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Imported")
            .to_string();
        return Ok(Some(Crosshair::new(name, layers)));
    }
    // A single bare layer.
    if map.contains_key("dot") && map.contains_key("line") {
        let layer: Layer = serde_json::from_value(Value::Object(map.clone()))
            .map_err(|e| format!("invalid crosshair layer: {e}"))?;
        return Ok(Some(Crosshair::new("Imported", vec![layer])));
    }
    Ok(None)
}

/// Read a preset file from disk.
pub fn import_file(path: &std::path::Path) -> Result<Vec<Crosshair>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    import_str(&text)
}

/// Turn a preset name into a safe file name stem.
pub fn file_stem(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else if ch.is_whitespace() {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    let collapsed = {
        let mut result = String::with_capacity(trimmed.len());
        let mut last_underscore = false;
        for ch in trimmed.chars() {
            if ch == '_' {
                if !last_underscore {
                    result.push(ch);
                }
                last_underscore = true;
            } else {
                result.push(ch);
                last_underscore = false;
            }
        }
        result
    };
    if collapsed.is_empty() {
        "crosshair".to_string()
    } else {
        collapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Direction, DotShape, LineShape};

    const COMMUNITY_SAMPLE: &str = r##"{
      "items": [
        {
          "crosshairId": [
            {
              "dot": {
                "diameter": 4,
                "opacity": 1,
                "color": "#FF0000",
                "shape": "ring",
                "thickness": 1,
                "blur": 0
              },
              "line": {
                "length": 8,
                "thickness": 2,
                "offset": 8,
                "opacity": 1,
                "color": "Red",
                "shape": "rectangle",
                "blur": 0,
                "rotation": 0,
                "visible": ["left", "right", "top", "bottom"]
              },
              "outline": {
                "thickness": 0,
                "opacity": 1,
                "color": "Black",
                "blur": 0
              },
              "firingOptions": {
                "tShapeWhenFiring": false,
                "firingOffset": 0,
                "mouseButton": "left",
                "loop": false
              },
              "position": { "x": 0, "y": 0 },
              "layer": { "size": 1 },
              "migrationVersion": 1
            }
          ],
          "name": "AK-47",
          "timestamp": 1607035094735
        }
      ]
    }"##;

    #[test]
    fn imports_community_page_payload() {
        let presets = import_str(COMMUNITY_SAMPLE).expect("parses");
        assert_eq!(presets.len(), 1);
        let p = &presets[0];
        assert_eq!(p.name, "AK-47");
        assert_eq!(p.layers.len(), 1);
        let layer = &p.layers[0];
        assert_eq!(layer.dot.shape, DotShape::Ring);
        assert_eq!(layer.line.shape, LineShape::Rectangle);
        assert_eq!(layer.line.offset, 8.0);
        assert_eq!(layer.line.visible.len(), 4);
        assert_eq!(layer.outline.thickness, 0.0);
    }

    #[test]
    fn imports_community_array_of_crosshairs() {
        let text = r##"[
          {"crosshairId": [{"dot": {"diameter": 2, "opacity": 1, "color": "#fff", "shape": "square"}}], "name": "A"},
          {"crosshairId": [], "name": "B"}
        ]"##;
        let presets = import_str(text).expect("parses");
        assert_eq!(presets.len(), 2);
        assert_eq!(presets[1].name, "B");
        // an empty layer list is repaired by sanitize
        assert_eq!(presets[1].layers.len(), 1);
    }

    #[test]
    fn round_trips_native_format() {
        let mut original = Crosshair::default();
        original.name = "Test".to_string();
        original.layers[0].line.visible = vec![Direction::Top, Direction::Bottom];
        original.layers[0].line.shape = LineShape::Triangle;
        original.layers[0].dot.shape = DotShape::Diamond;
        let text = export_json(&original);
        assert!(text.contains(FORMAT));
        let parsed = import_str(&text).expect("round trip");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0], original);
    }

    #[test]
    fn rejects_garbage() {
        assert!(import_str("not json").is_err());
        assert!(import_str("{\"foo\": 1}").is_err());
        assert!(import_str("[]").is_err());
    }

    #[test]
    fn file_stems_are_filesystem_safe() {
        assert_eq!(file_stem("AK-47 // red"), "AK-47_red");
        assert_eq!(file_stem("  "), "crosshair");
    }
}

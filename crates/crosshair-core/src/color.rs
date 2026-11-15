//! Small CSS-flavoured colour parser.
//!
//! Preset files in the wild store colours as `#RRGGBB`, `#RGB`, `rgb(r, g, b)`,
//! `rgba(...)` and a handful of CSS names. Anything unparseable falls back to
//! opaque white so a broken preset still renders.

/// Parsed colour: straight (non-premultiplied) RGBA, 0-255.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    pub const WHITE: Rgba8 = Rgba8 {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };

    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn with_alpha_scale(self, scale: f32) -> Self {
        let a = (self.a as f32 * scale.clamp(0.0, 1.0)).round().clamp(0.0, 255.0) as u8;
        Self { a, ..self }
    }

    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

/// Parse a colour string. Returns `None` for empty/unknown input.
pub fn parse(input: &str) -> Option<Rgba8> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    let lower = s.to_ascii_lowercase();

    if lower == "transparent" {
        return Some(Rgba8::new(0, 0, 0, 0));
    }
    if let Some(rest) = lower.strip_prefix('#') {
        let hex: String = rest.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        return match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                Some(Rgba8::new(r, g, b, 255))
            }
            4 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                let a = u8::from_str_radix(&hex[3..4].repeat(2), 16).ok()?;
                Some(Rgba8::new(r, g, b, a))
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Rgba8::new(r, g, b, 255))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Rgba8::new(r, g, b, a))
            }
            _ => None,
        };
    }

    if lower.starts_with("rgb") {
        let (body, mul) = if let Some(open) = lower.find('(') {
            let inner = lower[open + 1..].trim_end_matches(')');
            (inner.to_string(), 255.0f32)
        } else {
            return None;
        };
        let parts: Vec<&str> = body.split([',', ' ', '/']).filter(|p| !p.is_empty()).collect();
        if parts.len() < 3 {
            return None;
        }
        let channel = |p: &str| -> Option<u8> {
            let p = p.trim();
            let value = if let Some(pct) = p.strip_suffix('%') {
                pct.parse::<f32>().ok()? / 100.0 * 255.0
            } else {
                p.parse::<f32>().ok()?
            };
            Some(value.round().clamp(0.0, 255.0) as u8)
        };
        let r = channel(parts[0])?;
        let g = channel(parts[1])?;
        let b = channel(parts[2])?;
        let a = if parts.len() >= 4 {
            let p = parts[3].trim();
            let raw = p.parse::<f32>().ok()?;
            // CSS allows 0-1 alpha and 0-100% alpha in the same position.
            let unit = if p.ends_with('%') { raw / 100.0 } else { raw };
            (unit.clamp(0.0, 1.0) * mul).round() as u8
        } else {
            255
        };
        return Some(Rgba8::new(r, g, b, a));
    }

    named(&lower)
}

fn named(name: &str) -> Option<Rgba8> {
    let rgb = match name {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "red" => (255, 0, 0),
        "lime" | "green" => (0, 255, 0),
        "blue" => (0, 0, 255),
        "yellow" => (255, 255, 0),
        "cyan" | "aqua" => (0, 255, 255),
        "magenta" | "fuchsia" => (255, 0, 255),
        "orange" => (255, 165, 0),
        "purple" => (128, 0, 128),
        "pink" => (255, 192, 203),
        "gray" | "grey" => (128, 128, 128),
        "silver" => (192, 192, 192),
        "maroon" => (128, 0, 0),
        "navy" => (0, 0, 128),
        "teal" => (0, 128, 128),
        "olive" => (128, 128, 0),
        _ => return None,
    };
    Some(Rgba8::new(rgb.0, rgb.1, rgb.2, 255))
}

/// Parse with a white fallback, matching the reference renderer's behaviour.
pub fn parse_or_white(input: &str) -> Rgba8 {
    parse(input).unwrap_or(Rgba8::WHITE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_variants() {
        assert_eq!(parse("#ff0000"), Some(Rgba8::new(255, 0, 0, 255)));
        assert_eq!(parse("#FF0000"), Some(Rgba8::new(255, 0, 0, 255)));
        assert_eq!(parse("#f00"), Some(Rgba8::new(255, 0, 0, 255)));
        assert_eq!(parse("#00FF0080"), Some(Rgba8::new(0, 255, 0, 128)));
    }

    #[test]
    fn parses_rgb_and_rgba() {
        assert_eq!(parse("rgb(255, 0, 0)"), Some(Rgba8::new(255, 0, 0, 255)));
        assert_eq!(parse("rgba(1,2,3,0.5)"), Some(Rgba8::new(1, 2, 3, 128)));
        assert_eq!(parse("rgb(100%, 0%, 0%)"), Some(Rgba8::new(255, 0, 0, 255)));
    }

    #[test]
    fn parses_names_and_transparent() {
        assert_eq!(parse_or_white("Red"), Rgba8::new(255, 0, 0, 255));
        assert_eq!(parse_or_white("Black"), Rgba8::new(0, 0, 0, 255));
        assert_eq!(parse("transparent").unwrap().a, 0);
        assert_eq!(parse(""), None);
        assert_eq!(parse_or_white("nonsense"), Rgba8::WHITE);
    }

    #[test]
    fn alpha_scale_is_applied() {
        let c = Rgba8::new(10, 20, 30, 200).with_alpha_scale(0.5);
        assert_eq!(c.a, 100);
        assert_eq!(c.to_hex(), "#0A141E");
    }
}

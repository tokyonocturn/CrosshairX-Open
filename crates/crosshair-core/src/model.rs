//! Crosshair data model.
//!
//! The field names mirror the JSON schema used by the original Crosshair X presets
//! (`dot`, `line`, `outline`, `position`, `layer`) so that community payloads can be
//! imported losslessly, but the implementation is original.

use serde::{Deserialize, Serialize};

/// Shape of the centre dot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum DotShape {
    #[default]
    Square,
    Circle,
    Ring,
    Diamond,
}

impl DotShape {
    pub const ALL: [DotShape; 4] = [
        DotShape::Square,
        DotShape::Circle,
        DotShape::Ring,
        DotShape::Diamond,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DotShape::Square => "Square",
            DotShape::Circle => "Circle",
            DotShape::Ring => "Ring",
            DotShape::Diamond => "Diamond",
        }
    }
}

/// Shape of the four crosshair lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum LineShape {
    #[default]
    Rectangle,
    Triangle,
}

impl LineShape {
    pub const ALL: [LineShape; 2] = [LineShape::Rectangle, LineShape::Triangle];

    pub fn label(self) -> &'static str {
        match self {
            LineShape::Rectangle => "Rectangle",
            LineShape::Triangle => "Triangle",
        }
    }
}

/// One of the four line directions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Top,
    Bottom,
    Left,
    Right,
}

impl Direction {
    pub const ALL: [Direction; 4] = [
        Direction::Top,
        Direction::Bottom,
        Direction::Left,
        Direction::Right,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Direction::Top => "Top",
            Direction::Bottom => "Bottom",
            Direction::Left => "Left",
            Direction::Right => "Right",
        }
    }
}

/// A point in crosshair space. X grows right, Y grows up (screen convention is
/// handled by the renderer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Centre dot settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dot {
    pub diameter: f32,
    pub opacity: f32,
    pub color: String,
    pub shape: DotShape,
    /// Ring stroke width. Ignored by the filled shapes.
    pub thickness: f32,
    pub blur: f32,
}

impl Default for Dot {
    fn default() -> Self {
        Self {
            diameter: 4.0,
            opacity: 0.0,
            color: "#FF0000".to_string(),
            shape: DotShape::Square,
            thickness: 1.0,
            blur: 0.0,
        }
    }
}

/// Line (arm) settings. All four arms share one thickness/length/gap/colour;
/// `visible` selects which arms are drawn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Line {
    pub length: f32,
    pub thickness: f32,
    /// Distance between the centre and the inner edge of each arm.
    pub offset: f32,
    pub opacity: f32,
    pub color: String,
    pub shape: LineShape,
    /// Rotation of the whole arm group, in degrees, clockwise.
    pub rotation: f32,
    pub blur: f32,
    pub visible: Vec<Direction>,
}

impl Default for Line {
    fn default() -> Self {
        Self {
            length: 8.0,
            thickness: 2.0,
            offset: 4.0,
            opacity: 1.0,
            color: "#FF0000".to_string(),
            shape: LineShape::Rectangle,
            rotation: 0.0,
            blur: 0.0,
            visible: Direction::ALL.to_vec(),
        }
    }
}

impl Line {
    pub fn is_visible(&self, dir: Direction) -> bool {
        self.visible.contains(&dir)
    }

    /// True when the arms would draw at least one pixel.
    pub fn draws(&self) -> bool {
        self.opacity > 0.0 && self.length > 0.0 && self.thickness > 0.0 && !self.visible.is_empty()
    }
}

/// Outline drawn around the dot and around each arm.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Outline {
    pub thickness: f32,
    pub opacity: f32,
    pub color: String,
    pub blur: f32,
}

impl Default for Outline {
    fn default() -> Self {
        Self {
            thickness: 1.0,
            opacity: 1.0,
            color: "#000000".to_string(),
            blur: 0.0,
        }
    }
}

impl Outline {
    pub fn draws(&self) -> bool {
        self.thickness > 0.0 && self.opacity > 0.0
    }
}

/// Per-layer scale, stored under the `layer` key of the preset schema.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayerScale {
    pub size: f32,
}

impl Default for LayerScale {
    fn default() -> Self {
        Self { size: 1.0 }
    }
}

/// A single crosshair layer. A preset can stack several of these to build
/// composite designs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layer {
    pub dot: Dot,
    pub line: Line,
    pub outline: Outline,
    pub position: Point,
    pub layer: LayerScale,
}

impl Default for Layer {
    fn default() -> Self {
        Self {
            dot: Dot {
                opacity: 0.0,
                ..Dot::default()
            },
            line: Line::default(),
            outline: Outline {
                thickness: 1.0,
                ..Outline::default()
            },
            position: Point::ZERO,
            layer: LayerScale::default(),
        }
    }
}

impl Layer {
    /// A classic four-arm crosshair with a 1px outline.
    pub fn classic() -> Self {
        Self::default()
    }

    /// A single small dot with no arms.
    pub fn dot_only(diameter: f32, color: &str) -> Self {
        let mut layer = Self::default();
        layer.line.visible.clear();
        layer.dot.opacity = 1.0;
        layer.dot.diameter = diameter;
        layer.dot.shape = DotShape::Circle;
        layer.dot.color = color.to_string();
        layer
    }

    /// A four-arm crosshair with a gap and a dot, the shape most shooters use.
    pub fn cross_with_gap(length: f32, thickness: f32, gap: f32) -> Self {
        let mut layer = Self::default();
        layer.line.length = length;
        layer.line.thickness = thickness;
        layer.line.offset = gap;
        layer.dot.opacity = 0.0;
        layer
    }
}

/// A complete, named design.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Crosshair {
    pub name: String,
    pub layers: Vec<Layer>,
}

impl Default for Crosshair {
    fn default() -> Self {
        Self {
            name: "Untitled".to_string(),
            layers: vec![Layer::classic()],
        }
    }
}

impl Crosshair {
    pub fn new(name: impl Into<String>, layers: Vec<Layer>) -> Self {
        Self {
            name: name.into(),
            layers,
        }
    }

    pub fn single(name: impl Into<String>, layer: Layer) -> Self {
        Self::new(name, vec![layer])
    }

    /// Clamp every value into the range the editor exposes. Imported presets and
    /// hand-edited files can contain anything, so this runs after every load.
    pub fn sanitize(&mut self) {
        if self.layers.is_empty() {
            self.layers.push(Layer::classic());
        }
        if self.layers.len() > 16 {
            self.layers.truncate(16);
        }
        if self.name.trim().is_empty() {
            self.name = "Untitled".to_string();
        }
        for layer in &mut self.layers {
            layer.dot.diameter = clamp(layer.dot.diameter, 0.0, 400.0);
            layer.dot.thickness = clamp(layer.dot.thickness, 0.0, 200.0);
            layer.dot.opacity = clamp(layer.dot.opacity, 0.0, 1.0);
            layer.dot.blur = clamp(layer.dot.blur, 0.0, 20.0);
            layer.line.length = clamp(layer.line.length, 0.0, 400.0);
            layer.line.thickness = clamp(layer.line.thickness, 0.0, 200.0);
            layer.line.offset = clamp(layer.line.offset.abs(), 0.0, 400.0);
            layer.line.opacity = clamp(layer.line.opacity, 0.0, 1.0);
            layer.line.blur = clamp(layer.line.blur, 0.0, 20.0);
            layer.line.rotation = normalize_rotation(layer.line.rotation);
            layer.outline.thickness = clamp(layer.outline.thickness, 0.0, 50.0);
            layer.outline.opacity = clamp(layer.outline.opacity, 0.0, 1.0);
            layer.outline.blur = clamp(layer.outline.blur, 0.0, 20.0);
            layer.position.x = clamp(layer.position.x, -4096.0, 4096.0);
            layer.position.y = clamp(layer.position.y, -4096.0, 4096.0);
            layer.layer.size = clamp(layer.layer.size, 0.05, 20.0);
            let mut seen = Vec::new();
            layer.line.visible.retain(|d| {
                if seen.contains(d) {
                    false
                } else {
                    seen.push(*d);
                    true
                }
            });
        }
    }
}

pub fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    if v.is_nan() {
        lo
    } else {
        v.max(lo).min(hi)
    }
}

pub fn normalize_rotation(deg: f32) -> f32 {
    if !deg.is_finite() {
        return 0.0;
    }
    let mut r = deg % 360.0;
    if r < 0.0 {
        r += 360.0;
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_crosshair_is_a_four_arm_cross() {
        let crosshair = Crosshair::default();
        assert_eq!(crosshair.layers.len(), 1);
        let layer = &crosshair.layers[0];
        assert_eq!(layer.line.visible.len(), 4);
        assert!(layer.line.draws());
    }

    #[test]
    fn sanitize_rejects_nonsense_values() {
        let mut crosshair = Crosshair {
            name: "  ".to_string(),
            layers: vec![Layer {
                dot: Dot {
                    diameter: f32::NAN,
                    opacity: 42.0,
                    ..Dot::default()
                },
                line: Line {
                    rotation: -90.0,
                    visible: vec![Direction::Top, Direction::Top],
                    ..Line::default()
                },
                ..Layer::default()
            }],
        };
        crosshair.sanitize();
        assert_eq!(crosshair.name, "Untitled");
        assert_eq!(crosshair.layers[0].dot.diameter, 0.0);
        assert_eq!(crosshair.layers[0].dot.opacity, 1.0);
        assert_eq!(crosshair.layers[0].line.rotation, 270.0);
        assert_eq!(crosshair.layers[0].line.visible, vec![Direction::Top]);
    }

    #[test]
    fn sanitize_replaces_empty_layer_list() {
        let mut crosshair = Crosshair {
            name: "x".into(),
            layers: Vec::new(),
        };
        crosshair.sanitize();
        assert_eq!(crosshair.layers.len(), 1);
    }
}

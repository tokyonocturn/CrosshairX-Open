//! Crosshair geometry.
//!
//! Model space has the centre at (0, 0), +X to the right and +Y upwards. The
//! renderer flips Y so the overlay can use ordinary screen coordinates.
//!
//! The arm/outline layout follows the same rules the shipped Crosshair X
//! renderer uses (verified against its emitted SVG): each arm is a rectangle
//! whose inner edge sits `offset` px from the centre and whose length runs
//! outward, and the outline is a four-strip frame around that rectangle (or a
//! stroke around the triangle).

use crate::model::{Direction, Layer, Line, LineShape, Outline};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl LineRect {
    pub fn x2(&self) -> f32 {
        self.x + self.w
    }

    pub fn y2(&self) -> f32 {
        self.y + self.h
    }
}

/// Rectangle for one arm, in model space.
pub fn line_rect(line: &Line, dir: Direction) -> LineRect {
    let t = line.thickness.max(0.0);
    let len = line.length.max(0.0);
    let gap = line.offset.max(0.0);
    let half = t / 2.0;
    match dir {
        Direction::Top => LineRect {
            x: -half,
            y: -(gap + len),
            w: t,
            h: len,
        },
        Direction::Bottom => LineRect {
            x: -half,
            y: gap,
            w: t,
            h: len,
        },
        Direction::Left => LineRect {
            x: -(gap + len),
            y: -half,
            w: len,
            h: t,
        },
        Direction::Right => LineRect {
            x: gap,
            y: -half,
            w: len,
            h: t,
        },
    }
}

/// Triangle arm: tip nearest the centre, base facing outward.
pub fn triangle_points(line: &Line, dir: Direction) -> [(f32, f32); 3] {
    let t = line.thickness.max(0.0);
    let len = line.length.max(0.0);
    let gap = line.offset.max(0.0);
    let half = t / 2.0;
    match dir {
        Direction::Top => [(0.0, -gap), (-half, -(gap + len)), (half, -(gap + len))],
        Direction::Bottom => [(0.0, gap), (-half, gap + len), (half, gap + len)],
        Direction::Left => [(-gap, 0.0), (-(gap + len), -half), (-(gap + len), half)],
        Direction::Right => [(gap, 0.0), (gap + len, -half), (gap + len, half)],
    }
}

/// Four strips forming a frame of `outline.thickness` around an arm.
pub fn outline_rects(line: &Line, outline: &Outline, dir: Direction) -> Vec<LineRect> {
    let r = line_rect(line, dir);
    let t = outline.thickness.max(0.0);
    if t == 0.0 {
        return Vec::new();
    }
    vec![
        LineRect {
            x: r.x - t,
            y: r.y - t,
            w: r.w + 2.0 * t,
            h: t,
        },
        LineRect {
            x: r.x - t,
            y: r.y2(),
            w: r.w + 2.0 * t,
            h: t,
        },
        LineRect {
            x: r.x - t,
            y: r.y,
            w: t,
            h: r.h,
        },
        LineRect {
            x: r.x2(),
            y: r.y,
            w: t,
            h: r.h,
        },
    ]
}

/// Radius (model units) of the arm group, ignoring layer scale.
pub fn arm_radius(line: &Line) -> f32 {
    if !line.draws() {
        return 0.0;
    }
    let reach = line.offset.max(0.0) + line.length.max(0.0);
    let half = line.thickness.max(0.0) / 2.0;
    // A rotated arm group can reach the diagonal of its own rectangle.
    if line.rotation % 90.0 != 0.0 {
        (reach * reach + half * half).sqrt()
    } else {
        reach
    }
}

/// Half extent of a single layer in model units (before the global scale).
///
/// `blur` is the widest blur radius in the layer; blurred pixels bleed outside
/// the geometric outline so the canvas has to grow to fit them.
pub fn layer_half_extent(layer: &Layer) -> f32 {
    let arms = arm_radius(&layer.line);
    let dot = if layer.dot.opacity > 0.0 {
        layer.dot.diameter.max(0.0) / 2.0
    } else {
        0.0
    };
    let ring = if layer.dot.opacity > 0.0 && layer.dot.shape == crate::model::DotShape::Ring {
        layer.dot.thickness.max(0.0) / 2.0
    } else {
        0.0
    };
    let outline = if layer.outline.draws() {
        layer.outline.thickness.max(0.0)
    } else {
        0.0
    };
    let content = (arms.max(dot + ring) + outline) * layer.layer.size.max(0.0);
    let shift = layer.position.x.abs().max(layer.position.y.abs());
    content + shift + blur_pad(layer)
}

/// Extra padding needed because of blur, in model units.
pub fn blur_pad(layer: &Layer) -> f32 {
    let b = layer
        .dot
        .blur
        .max(layer.line.blur)
        .max(layer.outline.blur)
        .max(0.0);
    if b > 0.0 {
        b * 3.0
    } else {
        0.0
    }
}

/// Half extent of a whole design in model units.
pub fn crosshair_half_extent(layers: &[Layer]) -> f32 {
    layers.iter().map(layer_half_extent).fold(0.0f32, f32::max)
}

/// Translate a model-space point into image space for a canvas of `size`.
pub fn to_image(x: f32, y: f32, size: f32) -> (f32, f32) {
    (size / 2.0 + x, size / 2.0 - y)
}

/// Whether an arm should be drawn for `dir`.
pub fn arm_visible(line: &Line, dir: Direction) -> bool {
    line.draws() && line.is_visible(dir)
}

/// Convenience: the rectangle kind actually used for an arm.
pub fn arm_kind(line: &Line) -> LineShape {
    line.shape
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Layer;

    fn line() -> Line {
        Line {
            length: 10.0,
            thickness: 2.0,
            offset: 4.0,
            ..Line::default()
        }
    }

    #[test]
    fn rectangle_arms_have_expected_edges() {
        let l = line();
        let top = line_rect(&l, Direction::Top);
        assert_eq!(
            top,
            LineRect {
                x: -1.0,
                y: -14.0,
                w: 2.0,
                h: 10.0
            }
        );
        let bottom = line_rect(&l, Direction::Bottom);
        assert_eq!(
            bottom,
            LineRect {
                x: -1.0,
                y: 4.0,
                w: 2.0,
                h: 10.0
            }
        );
        let left = line_rect(&l, Direction::Left);
        assert_eq!(
            left,
            LineRect {
                x: -14.0,
                y: -1.0,
                w: 10.0,
                h: 2.0
            }
        );
        let right = line_rect(&l, Direction::Right);
        assert_eq!(
            right,
            LineRect {
                x: 4.0,
                y: -1.0,
                w: 10.0,
                h: 2.0
            }
        );
    }

    #[test]
    fn gap_is_respected_on_every_arm() {
        let l = Line {
            offset: 12.0,
            ..line()
        };
        assert_eq!(line_rect(&l, Direction::Right).x, 12.0);
        assert_eq!(line_rect(&l, Direction::Left).x2(), -12.0);
        assert_eq!(line_rect(&l, Direction::Top).y2(), -12.0);
        assert_eq!(line_rect(&l, Direction::Bottom).y, 12.0);
    }

    #[test]
    fn zero_length_or_thickness_draws_nothing() {
        let mut l = line();
        l.length = 0.0;
        assert!(!l.draws());
        l.length = 10.0;
        l.thickness = 0.0;
        assert!(!l.draws());
        l.thickness = 2.0;
        l.opacity = 0.0;
        assert!(!l.draws());
    }

    #[test]
    fn triangle_tip_faces_the_centre() {
        let l = line();
        let top = triangle_points(&l, Direction::Top);
        assert_eq!(top[0], (0.0, -4.0));
        assert_eq!(top[1], (-1.0, -14.0));
        assert_eq!(top[2], (1.0, -14.0));
        let right = triangle_points(&l, Direction::Right);
        assert_eq!(right[0], (4.0, 0.0));
    }

    #[test]
    fn outline_frames_the_arm() {
        let l = line();
        let o = Outline {
            thickness: 1.0,
            ..Outline::default()
        };
        let rects = outline_rects(&l, &o, Direction::Right);
        assert_eq!(rects.len(), 4);
        // top strip sits one pixel above the arm and spans its width plus both sides
        assert_eq!(
            rects[0],
            LineRect {
                x: 3.0,
                y: -2.0,
                w: 12.0,
                h: 1.0
            }
        );
        // bottom strip sits one pixel below
        assert_eq!(
            rects[1],
            LineRect {
                x: 3.0,
                y: 1.0,
                w: 12.0,
                h: 1.0
            }
        );
    }

    #[test]
    fn extent_covers_the_longest_arm() {
        let layer = Layer::default();
        // length 8, offset 4, outline 1 -> 13
        assert!((layer_half_extent(&layer) - 13.0).abs() < 0.001);
    }

    #[test]
    fn extent_grows_with_rotation() {
        let mut layer = Layer::default();
        let straight = layer_half_extent(&layer);
        layer.line.rotation = 45.0;
        assert!(layer_half_extent(&layer) > straight);
    }

    #[test]
    fn position_shifts_extent() {
        let mut layer = Layer::default();
        let base = layer_half_extent(&layer);
        layer.position = crate::model::Point::new(7.0, -3.0);
        assert!((layer_half_extent(&layer) - (base + 7.0)).abs() < 0.001);
    }

    #[test]
    fn image_projection_flips_y() {
        assert_eq!(to_image(0.0, 0.0, 100.0), (50.0, 50.0));
        assert_eq!(to_image(10.0, 10.0, 100.0), (60.0, 40.0));
    }
}

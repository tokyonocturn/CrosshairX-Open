//! Rasteriser for crosshair layers.
//!
//! Everything is drawn into a premultiplied RGBA pixmap, which is exactly the
//! format `UpdateLayeredWindow` wants, so the overlay can blit it without any
//! conversion. The editor preview uses the same function, so what you see in
//! the designer is what the overlay draws.

use tiny_skia::{
    FillRule, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform,
};

use crate::color::{self, Rgba8};
use crate::geom;
use crate::model::{Crosshair, Direction, DotShape, Layer, LineShape};

/// How to rasterise a design.
#[derive(Clone, Copy, Debug)]
pub struct RenderOptions {
    /// Multiplier applied to every model unit (size slider x DPI scale).
    pub scale: f32,
    /// Global opacity multiplier applied on top of per-element opacity.
    pub opacity: f32,
    /// Transparent margin around the drawing, in device pixels.
    pub padding: f32,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            scale: 1.0,
            opacity: 1.0,
            padding: 2.0,
        }
    }
}

/// A rasterised design plus the location of its centre inside the pixmap.
pub struct Rendered {
    pub pixmap: Pixmap,
    /// X of the crosshair centre in pixmap coordinates.
    pub center_x: f32,
    /// Y of the crosshair centre in pixmap coordinates.
    pub center_y: f32,
    /// Half extent of the drawing in device pixels (used to place the window).
    pub extent: f32,
}

impl Rendered {
    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }
}

fn paint(color: Rgba8, opacity: f32) -> Paint<'static> {
    let mut paint = Paint::default();
    let c = color.with_alpha_scale(opacity);
    paint.set_color_rgba8(c.r, c.g, c.b, c.a);
    paint.anti_alias = true;
    paint
}

fn rect_path(r: geom::LineRect) -> Option<Path> {
    let rect = Rect::from_xywh(r.x, r.y, r.w.max(0.0), r.h.max(0.0))?;
    let mut pb = PathBuilder::new();
    pb.push_rect(rect);
    pb.finish()
}

fn polygon_path(points: &[(f32, f32)]) -> Option<Path> {
    let mut pb = PathBuilder::new();
    let (x0, y0) = points[0];
    pb.move_to(x0, y0);
    for (x, y) in &points[1..] {
        pb.line_to(*x, *y);
    }
    pb.close();
    pb.finish()
}

fn circle_path(cx: f32, cy: f32, r: f32) -> Option<Path> {
    if r <= 0.0 {
        return None;
    }
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    pb.finish()
}

/// Rotation (degrees) then uniform scale then translation, in model units.
fn model_transform(tx: f32, ty: f32, scale: f32, rotation_deg: f32) -> Transform {
    let rad = rotation_deg.to_radians();
    let (sin, cos) = rad.sin_cos();
    Transform {
        sx: scale * cos,
        ky: scale * sin,
        kx: -scale * sin,
        sy: scale * cos,
        tx,
        ty,
    }
}

/// Apply a symmetric box blur (three passes approximate a Gaussian).
fn box_blur(pixmap: &mut Pixmap, radius: f32) {
    let radius = radius.round().max(1.0) as usize;
    if radius == 0 {
        return;
    }
    let w = pixmap.width() as usize;
    let h = pixmap.height() as usize;
    let mut src = pixmap.data().to_vec();
    let mut dst = vec![0u8; src.len()];
    for _ in 0..3 {
        // horizontal
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0u32; 4];
                let mut n = 0u32;
                let lo = x.saturating_sub(radius);
                let hi = (x + radius).min(w - 1);
                for xi in lo..=hi {
                    let i = (y * w + xi) * 4;
                    for c in 0..4 {
                        acc[c] += src[i + c] as u32;
                    }
                    n += 1;
                }
                let o = (y * w + x) * 4;
                for c in 0..4 {
                    dst[o + c] = (acc[c] / n.max(1)) as u8;
                }
            }
        }
        std::mem::swap(&mut src, &mut dst);
        // vertical
        for x in 0..w {
            for y in 0..h {
                let mut acc = [0u32; 4];
                let mut n = 0u32;
                let lo = y.saturating_sub(radius);
                let hi = (y + radius).min(h - 1);
                for yi in lo..=hi {
                    let i = (yi * w + x) * 4;
                    for c in 0..4 {
                        acc[c] += src[i + c] as u32;
                    }
                    n += 1;
                }
                let o = (y * w + x) * 4;
                for c in 0..4 {
                    dst[o + c] = (acc[c] / n.max(1)) as u8;
                }
            }
        }
        std::mem::swap(&mut src, &mut dst);
    }
    pixmap.data_mut().copy_from_slice(&src);
}

fn draw_blurred<F>(target: &mut Pixmap, blur: f32, scale: f32, draw: F)
where
    F: FnOnce(&mut Pixmap),
{
    if blur <= 0.0 {
        draw(target);
        return;
    }
    let Some(mut scratch) = Pixmap::new(target.width(), target.height()) else {
        draw(target);
        return;
    };
    draw(&mut scratch);
    box_blur(&mut scratch, blur * scale * 2.0);
    target.draw_pixmap(
        0,
        0,
        scratch.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
}

fn draw_dot(pixmap: &mut Pixmap, layer: &Layer, transform: Transform, opacity: f32) {
    let dot = &layer.dot;
    if dot.opacity <= 0.0 || dot.diameter <= 0.0 {
        return;
    }
    let r = dot.diameter / 2.0;
    let fill = paint(color::parse_or_white(&dot.color), dot.opacity * opacity);
    let outline = &layer.outline;
    let outline_opacity = if outline.draws() {
        outline.opacity * opacity
    } else {
        0.0
    };
    let ot = if outline_opacity > 0.0 {
        outline.thickness
    } else {
        0.0
    };
    let outline_paint = paint(color::parse_or_white(&outline.color), outline_opacity);
    let outline_filter = if outline.blur > 0.0 { outline.blur } else { 0.0 };

    let draw_shape = |pixmap: &mut Pixmap, color_paint: &Paint<'_>, expand: f32, stroked: bool| {
        match dot.shape {
            DotShape::Circle => {
                if let Some(path) = circle_path(0.0, 0.0, r + expand) {
                    pixmap.fill_path(&path, color_paint, FillRule::Winding, transform, None);
                }
            }
            DotShape::Ring => {
                // Ring with an outline is drawn as a wider stroke: the outline
                // colour bleeds symmetrically around the ring stroke.
                let stroke_w = dot.thickness.max(0.0) + 2.0 * expand;
                if let Some(path) = circle_path(0.0, 0.0, r) {
                    let mut stroke = Stroke::default();
                    stroke.width = stroke_w;
                    let mut p = color_paint.clone();
                    p.anti_alias = true;
                    pixmap.stroke_path(&path, &p, &stroke, transform, None);
                }
            }
            DotShape::Square | DotShape::Diamond => {
                let size = dot.diameter + 2.0 * expand;
                let rect = Rect::from_xywh(-size / 2.0, -size / 2.0, size, size);
                if let Some(rect) = rect {
                    let mut pb = PathBuilder::new();
                    pb.push_rect(rect);
                    if let Some(path) = pb.finish() {
                        let t = if dot.shape == DotShape::Diamond {
                            let rad = std::f32::consts::FRAC_PI_4;
                            Transform {
                                sx: rad.cos(),
                                ky: rad.sin(),
                                kx: -rad.sin(),
                                sy: rad.cos(),
                                tx: 0.0,
                                ty: 0.0,
                            }
                            .post_concat(transform)
                        } else {
                            transform
                        };
                        let _ = stroked;
                        pixmap.fill_path(&path, color_paint, FillRule::Winding, t, None);
                    }
                }
            }
        }
    };

    if ot > 0.0 {
        let filter_scale = outline_filter;
        draw_blurred(pixmap, filter_scale, transform.sx.abs(), |scratch| {
            draw_shape(scratch, &outline_paint, ot, true);
        });
    }
    let dot_blur = dot.blur;
    draw_blurred(pixmap, dot_blur, transform.sx.abs(), |scratch| {
        draw_shape(scratch, &fill, 0.0, false);
    });
}

fn draw_arms(pixmap: &mut Pixmap, layer: &Layer, transform: Transform, opacity: f32) {
    let line = &layer.line;
    if !line.draws() {
        return;
    }
    let outline = &layer.outline;
    let has_outline = outline.draws();
    let fill = paint(
        color::parse_or_white(&line.color),
        line.opacity * opacity,
    );
    let outline_paint = paint(
        color::parse_or_white(&outline.color),
        if has_outline {
            outline.opacity * opacity
        } else {
            0.0
        },
    );

    if has_outline {
        draw_blurred(pixmap, outline.blur, transform.sx.abs(), |scratch| {
            for dir in Direction::ALL {
                if !line.is_visible(dir) {
                    continue;
                }
                match line.shape {
                    LineShape::Rectangle => {
                        for r in geom::outline_rects(line, outline, dir) {
                            if let Some(path) = rect_path(r) {
                                scratch.fill_path(
                                    &path,
                                    &outline_paint,
                                    FillRule::Winding,
                                    transform,
                                    None,
                                );
                            }
                        }
                    }
                    LineShape::Triangle => {
                        if let Some(path) = polygon_path(&geom::triangle_points(line, dir)) {
                            let mut stroke = Stroke::default();
                            stroke.width = outline.thickness;
                            stroke.line_join = tiny_skia::LineJoin::Miter;
                            scratch.stroke_path(
                                &path,
                                &outline_paint,
                                &stroke,
                                transform,
                                None,
                            );
                        }
                    }
                }
            }
        });
    }

    draw_blurred(pixmap, line.blur, transform.sx.abs(), |scratch| {
        for dir in Direction::ALL {
            if !line.is_visible(dir) {
                continue;
            }
            match line.shape {
                LineShape::Rectangle => {
                    if let Some(path) = rect_path(geom::line_rect(line, dir)) {
                        scratch.fill_path(&path, &fill, FillRule::Winding, transform, None);
                    }
                }
                LineShape::Triangle => {
                    if let Some(path) = polygon_path(&geom::triangle_points(line, dir)) {
                        scratch.fill_path(&path, &fill, FillRule::Winding, transform, None);
                    }
                }
            }
        }
    });
}

fn draw_layer(pixmap: &mut Pixmap, layer: &Layer, scale: f32, opacity: f32, center: f32) {
    let size = layer.layer.size.max(0.0) * scale;
    if size <= 0.0 {
        return;
    }
    let tx = center + layer.position.x * scale;
    let ty = center - layer.position.y * scale;
    let arms_transform = model_transform(tx, ty, size, layer.line.rotation);
    let dot_transform = model_transform(tx, ty, size, 0.0);

    // Same stacking order as the reference renderer: arm outlines, dot
    // outline, dot, then arms.
    draw_arms(pixmap, layer, arms_transform, opacity);
    draw_dot(pixmap, layer, dot_transform, opacity);
}

/// Rasterise a design. Returns `None` only if the pixmap cannot be allocated.
pub fn render(crosshair: &Crosshair, opts: &RenderOptions) -> Option<Rendered> {
    let scale = opts.scale.max(0.01);
    let half_model = geom::crosshair_half_extent(&crosshair.layers);
    let padding = opts.padding.max(0.0);
    let half_px = (half_model * scale).ceil() + padding;
    let size = ((half_px * 2.0).ceil() as u32).clamp(8, 4096);
    let mut pixmap = Pixmap::new(size, size)?;
    let center = size as f32 / 2.0;
    for layer in &crosshair.layers {
        draw_layer(&mut pixmap, layer, scale, opts.opacity, center);
    }
    Some(Rendered {
        pixmap,
        center_x: center,
        center_y: center,
        extent: half_px,
    })
}

/// Rasterise a design at an explicit canvas size, centred. Used by the editor
/// preview so the preview does not jump around while dragging sliders.
pub fn render_sized(crosshair: &Crosshair, opts: &RenderOptions, size: u32) -> Option<Rendered> {
    let size = size.clamp(8, 4096);
    let mut pixmap = Pixmap::new(size, size)?;
    let center = size as f32 / 2.0;
    for layer in &crosshair.layers {
        draw_layer(&mut pixmap, layer, opts.scale, opts.opacity, center);
    }
    Some(Rendered {
        pixmap,
        center_x: center,
        center_y: center,
        extent: center,
    })
}

/// Allocate a pixmap and fill it with one colour (used by tests and the preview
/// background).
pub fn solid(size: u32, rgba: Rgba8) -> Option<Pixmap> {
    let mut pixmap = Pixmap::new(size, size)?;
    pixmap.fill(tiny_skia::Color::from_rgba8(
        rgba.r, rgba.g, rgba.b, rgba.a,
    ));
    Some(pixmap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DotShape, Layer};

    fn alpha_at(rendered: &Rendered, x: i32, y: i32) -> u8 {
        let w = rendered.width() as i32;
        if x < 0 || y < 0 || x >= w || y >= rendered.height() as i32 {
            return 0;
        }
        rendered.pixmap.data()[((y * w + x) * 4 + 3) as usize]
    }

    #[test]
    fn renders_a_visible_cross() {
        let crosshair = Crosshair::default();
        let rendered = render(&crosshair, &RenderOptions::default()).unwrap();
        let cx = rendered.center_x as i32;
        let cy = rendered.center_y as i32;
        // an arm sits a few pixels right of centre
        assert!(alpha_at(&rendered, cx + 6, cy) > 128, "right arm should be opaque");
        // the gap between centre and arm is empty for the default (gap 4, thickness 2)
        assert_eq!(alpha_at(&rendered, cx + 2, cy), 0);
        // far outside is empty
        assert_eq!(alpha_at(&rendered, 0, 0), 0);
    }

    #[test]
    fn hidden_direction_is_empty() {
        let mut layer = Layer::default();
        layer.line.visible = vec![Direction::Right];
        let crosshair = Crosshair::single("only-right", layer);
        let rendered = render(&crosshair, &RenderOptions::default()).unwrap();
        let cx = rendered.center_x as i32;
        let cy = rendered.center_y as i32;
        assert!(alpha_at(&rendered, cx + 6, cy) > 128);
        assert_eq!(alpha_at(&rendered, cx - 6, cy), 0);
        assert_eq!(alpha_at(&rendered, cx, cy - 6), 0);
    }

    #[test]
    fn opacity_scales_alpha() {
        let mut layer = Layer::default();
        layer.line.opacity = 0.5;
        let crosshair = Crosshair::single("half", layer);
        let rendered = render(&crosshair, &RenderOptions::default()).unwrap();
        let cx = rendered.center_x as i32;
        let cy = rendered.center_y as i32;
        let a = alpha_at(&rendered, cx + 6, cy);
        assert!((100..=140).contains(&a), "alpha was {a}");
    }

    #[test]
    fn global_opacity_is_applied() {
        let crosshair = Crosshair::default();
        let opts = RenderOptions {
            opacity: 0.25,
            ..RenderOptions::default()
        };
        let rendered = render(&crosshair, &opts).unwrap();
        let cx = rendered.center_x as i32;
        let cy = rendered.center_y as i32;
        assert!(alpha_at(&rendered, cx + 6, cy) < 90);
    }

    #[test]
    fn triangle_arms_render() {
        let mut layer = Layer::default();
        layer.line.shape = LineShape::Triangle;
        layer.line.length = 8.0;
        let crosshair = Crosshair::single("tri", layer);
        let rendered = render(&crosshair, &RenderOptions::default()).unwrap();
        let cx = rendered.center_x as i32;
        let cy = rendered.center_y as i32;
        assert!(alpha_at(&rendered, cx + 6, cy) > 0);
    }

    #[test]
    fn dot_shapes_render() {
        for shape in DotShape::ALL {
            let mut layer = Layer::default();
            layer.line.visible.clear();
            layer.dot.opacity = 1.0;
            layer.dot.diameter = 6.0;
            layer.dot.thickness = 2.0;
            layer.dot.shape = shape;
            let crosshair = Crosshair::single("dot", layer);
            let rendered = render(&crosshair, &RenderOptions::default()).unwrap();
            let cx = rendered.center_x as i32;
            let cy = rendered.center_y as i32;
            assert!(alpha_at(&rendered, cx, cy) > 0, "{shape:?} centre should be drawn");
        }
    }

    #[test]
    fn ring_is_hollow() {
        let mut layer = Layer::default();
        layer.line.visible.clear();
        layer.dot.opacity = 1.0;
        layer.dot.diameter = 12.0;
        layer.dot.thickness = 1.0;
        layer.dot.shape = DotShape::Ring;
        layer.outline.opacity = 0.0;
        let crosshair = Crosshair::single("ring", layer);
        let rendered = render(&crosshair, &RenderOptions::default()).unwrap();
        let cx = rendered.center_x as i32;
        let cy = rendered.center_y as i32;
        assert_eq!(alpha_at(&rendered, cx, cy), 0, "ring centre must be empty");
        assert!(alpha_at(&rendered, cx + 6, cy) > 0, "ring edge must be drawn");
    }

    #[test]
    fn blur_spreads_coverage() {
        let mut sharp_layer = Layer::default();
        sharp_layer.line.blur = 0.0;
        let sharp = render(
            &Crosshair::single("sharp", sharp_layer.clone()),
            &RenderOptions::default(),
        )
        .unwrap();
        let mut soft_layer = sharp_layer;
        soft_layer.line.blur = 2.0;
        let soft = render(
            &Crosshair::single("soft", soft_layer),
            &RenderOptions::default(),
        )
        .unwrap();
        let cx = sharp.center_x as i32;
        let cy = sharp.center_y as i32;
        // three pixels beyond the arm tip is empty when sharp, faint when blurred
        let tip = cx + 4 + 8 + 3;
        assert_eq!(alpha_at(&sharp, tip, cy), 0);
        assert!(alpha_at(&soft, tip, cy) > 0, "blurred arm should bleed outward");
    }

    #[test]
    fn size_scales_the_pixmap() {
        let crosshair = Crosshair::default();
        let small = render(&crosshair, &RenderOptions::default()).unwrap();
        let large = render(
            &crosshair,
            &RenderOptions {
                scale: 3.0,
                ..RenderOptions::default()
            },
        )
        .unwrap();
        assert!(large.width() > small.width() * 2);
    }

    #[test]
    fn layer_position_moves_the_drawing() {
        let mut layer = Layer::default();
        layer.position = crate::model::Point::new(10.0, 0.0);
        let crosshair = Crosshair::single("shifted", layer);
        let rendered = render(&crosshair, &RenderOptions::default()).unwrap();
        let cx = rendered.center_x as i32;
        let cy = rendered.center_y as i32;
        // the design moved 10px right: the gap now sits at +7..+13
        assert_eq!(alpha_at(&rendered, cx + 9, cy), 0);
        assert!(alpha_at(&rendered, cx + 16, cy) > 0);
    }

    #[test]
    fn layer_scale_multiplies() {
        let mut layer = Layer::default();
        layer.layer.size = 2.0;
        let scaled = Crosshair::single("scaled", layer);
        let rendered = render(&scaled, &RenderOptions::default()).unwrap();
        let cx = rendered.center_x as i32;
        let cy = rendered.center_y as i32;
        // arm now spans 8..24 px from centre (offset 4 and length 8, doubled)
        assert!(alpha_at(&rendered, cx + 20, cy) > 0);
        assert_eq!(alpha_at(&rendered, cx + 5, cy), 0);
    }

    #[test]
    fn transparent_design_still_renders() {
        let mut layer = Layer::default();
        layer.line.opacity = 0.0;
        layer.dot.opacity = 0.0;
        let rendered = render(&Crosshair::single("empty", layer), &RenderOptions::default())
            .expect("pixmap allocated");
        assert!(rendered.pixmap.data().iter().all(|b| *b == 0));
    }
}

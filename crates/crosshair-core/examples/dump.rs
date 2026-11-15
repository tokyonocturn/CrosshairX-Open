use crosshair_core::{render, Crosshair, Layer, Point, RenderOptions};

fn row(rendered: &crosshair_core::Rendered, y: u32) {
    let w = rendered.width();
    let data = rendered.pixmap.data();
    let mut s = String::new();
    for x in 0..w {
        let a = data[((y * w + x) * 4 + 3) as usize];
        s.push(if a > 200 { '#' } else if a > 20 { '+' } else if a > 0 { '.' } else { ' ' });
    }
    println!("|{s}| y={y} w={w} center={}", rendered.center_x);
}

fn main() {
    let mut layer = Layer::default();
    layer.position = Point::new(10.0, 0.0);
    let c = Crosshair::single("shifted", layer);
    let r = render(&c, &RenderOptions::default()).unwrap();
    row(&r, r.center_y as u32);
    let mut layer2 = Layer::default();
    layer2.layer.size = 2.0;
    let c2 = Crosshair::single("scaled", layer2);
    let r2 = render(&c2, &RenderOptions::default()).unwrap();
    row(&r2, r2.center_y as u32);
}

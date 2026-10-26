#![cfg(not(target_arch = "wasm32"))]

use solomon_graphics::raster::{Paint, Raster};
use solomon_sim::Vec2;

fn pixel(pixels: &[u8], width: usize, x: usize, y: usize) -> &[u8] {
    &pixels[(y * width + x) * 4..(y * width + x + 1) * 4]
}

#[test]
fn wall_strokes_keep_butt_caps_layer_order_and_straight_alpha() {
    let mut raster = Raster::new(64, 64).unwrap();
    let points = [Vec2::new(16.0, 32.0), Vec2::new(48.0, 32.0)];
    for (rgb, width) in [([16, 16, 20], 26.0), ([58, 58, 66], 20.0), ([85, 85, 94], 8.0)] {
        raster.stroke(&points, width, false, Paint::Color { rgb, alpha: 1.0 }).unwrap();
    }
    let pixels = raster.into_rgba().unwrap();
    assert_eq!(pixel(&pixels, 64, 32, 32), [85, 85, 94, 255]);
    assert_eq!(pixel(&pixels, 64, 32, 24), [58, 58, 66, 255]);
    assert_eq!(pixel(&pixels, 64, 32, 20), [16, 16, 20, 255]);
    assert_eq!(pixel(&pixels, 64, 15, 32), [0, 0, 0, 0]);
    let mut translucent = Raster::new(64, 64).unwrap();
    translucent.stroke(&points, 8.0, true, Paint::Color { rgb: [255, 0, 0], alpha: 0.5 }).unwrap();
    let pixels = translucent.into_rgba().unwrap();
    assert_eq!(pixel(&pixels, 64, 32, 32), [255, 0, 0, 128]);
    assert!(pixel(&pixels, 64, 14, 32)[3] > 0, "round terrain caps extend beyond the endpoint");
}

#[test]
fn repeating_terrain_pattern_keeps_world_phase_across_tile_origins() {
    let texture = image::RgbaImage::from_fn(3, 2, |x, _| {
        image::Rgba(if x == 0 { [255, 0, 0, 255] } else { [0, 255, 0, 255] })
    });
    let mut encoded = std::io::Cursor::new(Vec::new());
    texture.write_to(&mut encoded, image::ImageFormat::Png).unwrap();
    let image = solomon_graphics::assets::decode_image("pattern", encoded.get_ref()).unwrap();
    let tile = |origin: f64| {
        let mut raster = Raster::new(16, 16).unwrap();
        raster
            .stroke(
                &[Vec2::new(-16.0, 8.0), Vec2::new(32.0, 8.0)],
                8.0,
                true,
                Paint::RepeatingImage { image: &image, origin: Vec2::new(-origin, 0.0) },
            )
            .unwrap();
        raster.into_rgba().unwrap()
    };
    let first = tile(0.0);
    let second = tile(7.0);
    for x in 0..9 {
        assert_eq!(pixel(&first, 16, x + 7, 8), pixel(&second, 16, x, 8));
    }
    assert_ne!(pixel(&first, 16, 0, 8), pixel(&first, 16, 1, 8));
}

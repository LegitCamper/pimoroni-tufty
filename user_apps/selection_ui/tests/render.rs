use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use selection_ui::SelectionUi;

struct Screen {
    pixels: Vec<Rgb565>,
    outside: usize,
}

impl OriginDimensions for Screen {
    fn size(&self) -> Size {
        Size::new(320, 240)
    }
}

impl DrawTarget for Screen {
    type Color = Rgb565;
    type Error = ();
    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(&mut self, pixels: I) -> Result<(), ()> {
        for Pixel(point, color) in pixels {
            if (0..320).contains(&point.x) && (0..240).contains(&point.y) {
                self.pixels[point.y as usize * 320 + point.x as usize] = color;
            } else {
                self.outside += 1;
            }
        }
        Ok(())
    }
}

#[test]
fn library_and_empty_state_fit_badge_screen() {
    let mut screen = Screen {
        pixels: vec![Rgb565::BLACK; 320 * 240],
        outside: 0,
    };
    let files = [
        "/games/nes/Starfall.nes",
        "Neon Rally.nes",
        "Tiny Quest.nes",
        "Orbit Patrol.nes",
        "Pixel Harbor.nes",
        "A sixth game.nes",
    ];
    SelectionUi::new(&files)
        .with_title("NES / 8-BIT")
        .draw(&mut screen)
        .unwrap();
    assert_eq!(screen.outside, 0);
    assert_ne!(screen.pixels[55 * 320 + 13], screen.pixels[84 * 320 + 13]);
    if let Ok(path) = std::env::var("TUFTY_UI_PPM") {
        let mut bytes = b"P6\n320 240\n255\n".to_vec();
        for pixel in &screen.pixels {
            bytes.extend_from_slice(&[
                ((pixel.r() as u16 * 255) / 31) as u8,
                ((pixel.g() as u16 * 255) / 63) as u8,
                ((pixel.b() as u16 * 255) / 31) as u8,
            ]);
        }
        std::fs::write(path, bytes).unwrap();
    }
    SelectionUi::new(&[])
        .with_title("GAME BOY ADVANCE")
        .with_hint("/games/gba")
        .draw(&mut screen)
        .unwrap();
    assert_eq!(screen.outside, 0);
}

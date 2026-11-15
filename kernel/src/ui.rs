use crate::{
    APP_RUNNING, BINARY_CH,
    display::{FRAMEBUFFER, FrameBuffer},
    storage,
    syscalls::KEY_CACHE,
};
use alloc::{string::String, vec::Vec};
use core::sync::atomic::Ordering;
use embassy_time::Timer;
use embedded_graphics::{
    mono_font::{
        MonoTextStyle,
        ascii::{FONT_6X10, FONT_9X18},
    },
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};
use tufty_sys::{
    app,
    input::{KeyCode, KeyState},
    png,
};

enum App {
    Builtin(&'static crate::builtin::App),
    Elf(String),
}

fn rect(frame: &mut FrameBuffer, x: i32, y: i32, width: u32, height: u32, color: Rgb565) {
    let _ = Rectangle::new(Point::new(x, y), Size::new(width, height))
        .into_styled(PrimitiveStyle::with_fill(color))
        .draw(frame);
}
fn text(frame: &mut FrameBuffer, value: &str, x: i32, y: i32, color: Rgb565) {
    let _ = Text::new(
        value,
        Point::new(x, y),
        MonoTextStyle::new(&FONT_6X10, color),
    )
    .draw(frame);
}

async fn draw(apps: &[App], selected: usize, error: Option<&str>) {
    let mut guard = FRAMEBUFFER.lock().await;
    let Some(frame) = guard.as_mut() else {
        return;
    };
    let background = Rgb565::new(2, 5, 7);
    let accent = Rgb565::new(31, 47, 7);
    let _ = frame.clear(background);
    let _ = Text::new(
        "TUFTY / PLAY",
        Point::new(10, 22),
        MonoTextStyle::new(&FONT_9X18, Rgb565::WHITE),
    )
    .draw(frame);
    text(
        frame,
        if crate::input::GAMEPAD.load(Ordering::Acquire) {
            "PAD +"
        } else {
            "PAD -"
        },
        274,
        20,
        accent,
    );
    let page = selected / 6;
    for (slot, app) in apps.iter().skip(page * 6).take(6).enumerate() {
        let x = 8 + (slot % 3) as i32 * 104;
        let y = 36 + (slot / 3) as i32 * 85;
        let active = selected == page * 6 + slot;
        let color = if active {
            Rgb565::new(6, 15, 18)
        } else {
            Rgb565::new(3, 9, 12)
        };
        rect(frame, x, y, 96, 78, color);
        if active {
            rect(frame, x, y + 75, 96, 3, accent);
        }
        let (title, icon) = match app {
            App::Builtin(app) => (app.title, Some(app.icon)),
            App::Elf(path) => {
                let metadata = storage::file(path).ok().and_then(app::from_elf);
                match metadata {
                    Some(metadata) => (metadata.title, metadata.icon),
                    None => (
                        path.rsplit('/')
                            .next()
                            .unwrap_or(path)
                            .trim_end_matches(".bin"),
                        None,
                    ),
                }
            }
        };
        let icon = icon.and_then(|bytes| png::decode(bytes).ok());
        if let Some(icon) = icon {
            let left = x + (96 - icon.width as i32) / 2;
            for (index, rgba) in icon.rgba.chunks_exact(4).enumerate() {
                let alpha = rgba[3] as u32;
                let blend = |channel: u8, background: u8| {
                    ((channel as u32 * alpha + background as u32 * (255 - alpha)) / 255) as u8
                };
                let red = blend(rgba[0], ((color.r() as u16 * 255) / 31) as u8);
                let green = blend(rgba[1], ((color.g() as u16 * 255) / 63) as u8);
                let blue = blend(rgba[2], ((color.b() as u16 * 255) / 31) as u8);
                let pixel = Rgb565::new(red >> 3, green >> 2, blue >> 3);
                let _ = frame.draw_iter(core::iter::once(Pixel(
                    Point::new(
                        left + (index % icon.width) as i32,
                        y + 6 + (index / icon.width) as i32,
                    ),
                    pixel,
                )));
            }
        } else {
            rect(frame, x + 32, y + 14, 32, 24, accent);
            rect(frame, x + 37, y + 24, 10, 4, background);
            rect(frame, x + 40, y + 21, 4, 10, background);
            rect(frame, x + 54, y + 23, 4, 4, background);
        }
        let mut label = heapless::String::<16>::new();
        for character in title.chars().take(14) {
            let _ = label.push(if character.is_ascii() { character } else { '?' });
        }
        text(frame, &label, x + 6, y + 65, Rgb565::WHITE);
    }
    if apps.is_empty() {
        text(
            frame,
            "No apps. Build and flash an app pack.",
            10,
            108,
            Rgb565::WHITE,
        );
    }
    text(
        frame,
        error.unwrap_or("UP/DOWN browse   A play   B back"),
        8,
        224,
        accent,
    );
    let pages = crate::format!(32, "{}/{}", page + 1, apps.len().div_ceil(6).max(1));
    text(frame, &pages, 280, 238, Rgb565::WHITE);
}

pub async fn run() -> ! {
    while FRAMEBUFFER.lock().await.is_none() {
        Timer::after_millis(10).await;
    }
    let mut apps: Vec<App> = crate::builtin::APPS.iter().map(App::Builtin).collect();
    let mut error = None;
    match storage::pack() {
        Ok(pack) => {
            for index in 0..pack.len() {
                if let Ok((name, _)) = pack.entry(index) {
                    if name.to_ascii_lowercase().ends_with(".bin") && name.starts_with("/apps/") {
                        apps.push(App::Elf(String::from(name)));
                    }
                }
            }
        }
        Err(_) => error = Some("No game pack. Builtin apps available."),
    }
    let mut selected = 0usize;
    let mut changed = true;
    let mut pad = false;
    loop {
        let connected = crate::input::GAMEPAD.load(Ordering::Acquire);
        changed |= connected != pad;
        pad = connected;
        while let Ok(event) = KEY_CACHE.try_receive() {
            if event.state != KeyState::Pressed {
                continue;
            }
            match event.key {
                KeyCode::Up | KeyCode::Left if !apps.is_empty() => {
                    selected = (selected + apps.len() - 1) % apps.len();
                    error = None;
                }
                KeyCode::Down | KeyCode::Right if !apps.is_empty() => {
                    selected = (selected + 1) % apps.len();
                    error = None;
                }
                KeyCode::Enter if !apps.is_empty() => {
                    let program = match &apps[selected] {
                        App::Builtin(app) => Ok((app.entry, None, Default::default())),
                        App::Elf(path) => {
                            // SAFETY: layout validated; native code remains trusted, not sandboxed.
                            unsafe { crate::elf::load_binary(path).await }
                                .map(|(entry, image, region)| (entry, Some(image), region))
                        }
                    };
                    match program {
                        Ok(program) => {
                            while KEY_CACHE.try_receive().is_ok() {}
                            APP_RUNNING.store(true, Ordering::Release);
                            BINARY_CH.send(program).await;
                            while APP_RUNNING.load(Ordering::Acquire) {
                                Timer::after_millis(10).await;
                            }
                        }
                        Err(_) => error = Some("App failed. Check ELF and ABI version."),
                    }
                }
                _ => {}
            }
            changed = true;
        }
        if changed {
            draw(&apps, selected, error).await;
            changed = false;
        }
        Timer::after_millis(10).await;
    }
}

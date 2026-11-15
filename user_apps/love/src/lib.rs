#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::ffi::{c_char, c_void};
use embedded_graphics::{
    mono_font::{
        MonoFont, MonoTextStyle,
        ascii::{FONT_4X6, FONT_6X10},
    },
    pixelcolor::{Rgb565, raw::RawU16},
    prelude::*,
    text::{Baseline, Text},
};
use selection_ui::SelectionUi;
use tufty::{
    Buttons, PsramBuffer,
    display::Display,
    love::{MAX_SOURCE, validate_source},
};

type TextCallback = extern "C" fn(*mut u16, *const u8, usize, i32, i32, u16);

unsafe extern "C" {
    fn tufty_love_size() -> usize;
    fn tufty_love_init(
        memory: *mut c_void,
        source: *const u8,
        length: usize,
        text: TextCallback,
    ) -> i32;
    fn tufty_love_tick(memory: *mut c_void, buttons: u32, delta: f32) -> i32;
    fn tufty_pico8_init(
        memory: *mut c_void,
        source: *const u8,
        length: usize,
        text: TextCallback,
        assets: *const u8,
        asset_length: usize,
    ) -> i32;
    fn tufty_love_rate(memory: *mut c_void) -> u32;
    fn tufty_love_error(memory: *mut c_void) -> *const c_char;
    fn tufty_love_pixels(memory: *mut c_void) -> *const u16;
}

struct Canvas<'a>(&'a mut [u16]);

impl OriginDimensions for Canvas<'_> {
    fn size(&self) -> Size {
        Size::new(320, 240)
    }
}

impl DrawTarget for Canvas<'_> {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Rgb565>>,
    {
        for Pixel(point, color) in pixels {
            if (0..320).contains(&point.x) && (0..240).contains(&point.y) {
                self.0[point.y as usize * 320 + point.x as usize] = color.into_storage();
            }
        }
        Ok(())
    }
}

extern "C" fn paint_text(
    pixels: *mut u16,
    text: *const u8,
    length: usize,
    left: i32,
    top: i32,
    color: u16,
) {
    draw_text(pixels, text, length, left, top, color, &FONT_6X10);
}

extern "C" fn paint_pico_text(
    pixels: *mut u16,
    text: *const u8,
    length: usize,
    left: i32,
    top: i32,
    color: u16,
) {
    draw_text(pixels, text, length, left, top, color, &FONT_4X6);
}

fn draw_text(
    pixels: *mut u16,
    text: *const u8,
    length: usize,
    left: i32,
    top: i32,
    color: u16,
    font: &MonoFont<'_>,
) {
    // SAFETY: private C adapter supplies exclusive 320x240 pixels and checked live ASCII text.
    // This callback never calls Lua; Lua longjmp cannot cross a Rust frame.
    let (pixels, text) = unsafe {
        (
            core::slice::from_raw_parts_mut(pixels, 320 * 240),
            core::slice::from_raw_parts(text, length),
        )
    };
    if let Ok(text) = core::str::from_utf8(text) {
        let style = MonoTextStyle::new(font, Rgb565::from(RawU16::new(color)));
        let _ = Text::with_baseline(text, Point::new(left, top), style, Baseline::Top)
            .draw(&mut Canvas(pixels));
    }
}

fn show_error(display: &mut Display, message: &str) {
    selection_ui::notice(display, "LUA GAME ERROR", message);
    let mut previous = tufty::buttons();
    loop {
        let current = tufty::buttons();
        if (current - previous).intersects(Buttons::A | Buttons::B | Buttons::HOME) {
            return;
        }
        previous = current;
        tufty::sleep(10);
    }
}

pub fn run() {
    launch(false);
}

pub fn run_pico8() {
    launch(true);
}

fn launch(pico: bool) {
    let Some(mut display) = Display::take() else {
        return;
    };
    let (directory, extension, title, hint, demo, limit) = if pico {
        (
            "/games/pico8",
            "p8",
            "PICO-8 MINI",
            "/games/pico8/*.p8 - HOME exits",
            "STAR RUN / bundled demo",
            tufty::pico8::MAX_CART,
        )
    } else {
        (
            "/games/love",
            "lua",
            "LUA / LOVE MINI",
            "/games/love/*.lua - HOME exits",
            "ORBIT DODGE / bundled demo",
            MAX_SOURCE,
        )
    };
    let files = match tufty::fs::list_media(directory, extension) {
        Ok(files) => files,
        Err(_) => {
            show_error(&mut display, "Cannot read Lua game library");
            return;
        }
    };
    let mut labels = Vec::new();
    if labels.try_reserve_exact(files.len() + 1).is_err() {
        show_error(&mut display, "Not enough app SRAM");
        return;
    }
    labels.push(demo);
    labels.extend(files.iter().map(|path| path.as_str()));
    let Ok(Some(selected)) = SelectionUi::new(&labels)
        .with_title(title)
        .with_hint(hint)
        .run_selection_ui(&mut display)
    else {
        return;
    };
    let mut storage;
    let source = if selected == 0 {
        if pico {
            include_bytes!("../demo.p8").as_slice()
        } else {
            include_bytes!("../demo.lua").as_slice()
        }
    } else {
        let path = &files[selected - 1];
        let Ok(length) = tufty::fs::file_len(path) else {
            show_error(&mut display, "Cannot read script size");
            return;
        };
        if length == 0 || length > limit {
            show_error(&mut display, "Game file empty or exceeds format limit");
            return;
        }
        storage = match PsramBuffer::new(length) {
            Ok(buffer) => buffer,
            Err(_) => {
                show_error(&mut display, "Not enough PSRAM for script");
                return;
            }
        };
        if tufty::fs::read_file(path, 0, &mut storage) != Ok(length) {
            show_error(&mut display, "Short script read");
            return;
        }
        &storage
    };
    let result = if pico {
        play_cart(source, &mut display)
    } else {
        play(source, None, &mut display)
    };
    if let Err(message) = result {
        show_error(&mut display, message);
    }
}

fn play_cart(bytes: &[u8], display: &mut Display) -> Result<(), &'static str> {
    let cart = tufty::pico8::parse(bytes)?;
    let mut assets = PsramBuffer::new(tufty::pico8::ASSET_BYTES)
        .map_err(|_| "Not enough PSRAM for P8 assets")?;
    cart.decode_assets(&mut assets)?;
    let mut source =
        PsramBuffer::new(cart.source.len()).map_err(|_| "Not enough PSRAM for P8 source")?;
    tufty::pico8::normalize_source(cart.source, &mut source)?;
    play(&source, Some(&assets), display)
}

fn play(source: &[u8], assets: Option<&[u8]>, display: &mut Display) -> Result<(), &'static str> {
    validate_source(source)?;
    // SAFETY: linked C adapter reports its plain-data session size without side effects.
    let size = unsafe { tufty_love_size() };
    if size == 0 || size > 1024 * 1024 {
        return Err("Lua session exceeds 1 MiB budget");
    }
    let mut storage =
        PsramBuffer::new(size + 15).map_err(|_| "Not enough PSRAM for Lua session")?;
    let offset = storage.as_mut_ptr().align_offset(16);
    let state = storage[offset..offset + size].as_mut_ptr().cast::<c_void>();
    // SAFETY: aligned owned session and validated source remain live during every C call.
    // All Lua errors are caught inside C; Rust callbacks never invoke Lua.
    let mut status = unsafe {
        if let Some(assets) = assets {
            tufty_pico8_init(
                state,
                source.as_ptr(),
                source.len(),
                paint_pico_text,
                assets.as_ptr(),
                assets.len(),
            )
        } else {
            tufty_love_init(state, source.as_ptr(), source.len(), paint_text)
        }
    };
    // SAFETY: initialized session owns its rate, including failed initialization states.
    let rate = unsafe { tufty_love_rate(state) };
    if !matches!(rate, 30 | 60) {
        return Err("Invalid Lua frame rate");
    }
    let mut previous_time = tufty::get_ms();
    let mut deadline = previous_time;
    let mut remainder = 0u32;
    let mut strip = [0u16; 320 * 4];
    let mut launch_buttons = tufty::buttons();
    while status == 0 {
        let start = tufty::get_ms();
        let buttons = tufty::buttons();
        if buttons.contains(Buttons::HOME) {
            break;
        }
        launch_buttons &= buttons;
        let delta = start.saturating_sub(previous_time).clamp(1, 100) as f32 / 1000.0;
        previous_time = start;
        // SAFETY: session is live, exclusively accessed, and input values match C bounds.
        status = unsafe { tufty_love_tick(state, (buttons - launch_buttons).bits(), delta) };
        if status != 0 {
            break;
        }
        // SAFETY: C returns its initialized fixed-size framebuffer within this owned session.
        // The immutable view ends before the next mutation by Lua.
        let pixels = unsafe { core::slice::from_raw_parts(tufty_love_pixels(state), 320 * 240) };
        for (row, source_rows) in pixels.chunks_exact(320 * 4).enumerate() {
            strip.copy_from_slice(source_rows);
            display
                .blit(0, (row * 4) as u16, 320, 4, &strip)
                .ok_or("Display transfer failed")?;
        }
        remainder += 1000;
        deadline = deadline.saturating_add((remainder / rate) as u64);
        remainder %= rate;
        let now = tufty::get_ms();
        if deadline < now {
            deadline = now;
        }
        tufty::sleep(deadline.saturating_sub(now));
    }
    if status < 0 {
        // SAFETY: C guarantees a NUL-terminated error inside the still-live session.
        let message = unsafe { core::ffi::CStr::from_ptr(tufty_love_error(state)) };
        show_error(display, message.to_str().unwrap_or("Lua runtime error"));
    }
    // ponytail: no external handles or finalizers; reclaim entire session on return.
    // Add protected teardown if future APIs acquire resources outside this pool.
    Ok(())
}

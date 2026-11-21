#![no_std]
#![allow(static_mut_refs)]

extern crate alloc;

use alloc::vec::Vec;
use core::mem::MaybeUninit;
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb565,
    prelude::RgbColor,
};
use selection_ui::{SelectionUi, SelectionUiError, draw_text_center};
use tufty::{
    Buttons,
    display::Display,
    format,
    fs::{file_len, list_media, read_file, write_file},
    get_ms, println, sleep,
};

mod peanut;
mod video;
use peanut::gb_run_frame;

use crate::peanut::{
    JOYPAD_A, JOYPAD_B, JOYPAD_DOWN, JOYPAD_LEFT, JOYPAD_RIGHT, JOYPAD_SELECT, JOYPAD_START,
    JOYPAD_UP, ROM, gb_cart_ram_read, gb_cart_ram_write, gb_error, gb_get_rom_name,
    gb_get_save_size, gb_init, gb_init_lcd, gb_rom_read, gb_s, gb_set_frame_skip, gb_set_joypad,
    lcd_draw_line,
};

static mut DISPLAY: Option<Display> = None;

const RAM_SIZE: usize = 32 * 1024;
static mut RAM: *mut [u8] =
    core::ptr::slice_from_raw_parts_mut(core::ptr::NonNull::<u8>::dangling().as_ptr(), 0);

const GAME_PATH: &str = "/games/gameboy";
const CLOCK_HZ: u64 = 4_194_304;
const FRAME_TICKS: u64 = 70_224 * 1000;

struct Session {
    _ram: Vec<u8>,
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: all emulation has returned; clear callback pointers before freeing cartridge RAM.
        unsafe {
            RAM = core::ptr::slice_from_raw_parts_mut(
                core::ptr::NonNull::<u8>::dangling().as_ptr(),
                0,
            );
            DISPLAY = None;
        }
    }
}

struct Rom(tufty::PsramBuffer);
impl Drop for Rom {
    fn drop(&mut self) {
        // SAFETY: callbacks are synchronous; none can run while the ROM is dropped.
        unsafe {
            ROM = &[];
        }
    }
}

pub fn run() {
    let Some(display) = Display::take() else {
        return;
    };
    let mut ram = Vec::new();
    if ram.try_reserve_exact(RAM_SIZE).is_err() {
        return;
    }
    ram.resize(RAM_SIZE, 0);
    // SAFETY: one app runs on core1; Session owns RAM and display until callbacks stop.
    unsafe {
        RAM = core::ptr::slice_from_raw_parts_mut(ram.as_mut_ptr(), ram.len());
        DISPLAY = Some(display);
    }
    let _session = Session { _ram: ram };
    main();
}

fn main() {
    println!("Starting Gameboy app");

    if tufty::device_status() & tufty::wireless::STORAGE_READ_ONLY != 0 {
        // SAFETY: DISPLAY belongs exclusively to this app core.
        let display = unsafe { DISPLAY.as_mut().unwrap() };
        let _ = draw_text_center(
            display,
            "Saves do not persist.\nA: play   B/HOME: back",
            MonoTextStyle::new(&FONT_6X10, Rgb565::YELLOW),
        );
        while tufty::buttons().contains(Buttons::A) {
            sleep(10);
        }
        loop {
            let held = tufty::buttons();
            if held.intersects(Buttons::B | Buttons::HOME) {
                return;
            }
            if held.contains(Buttons::A) {
                break;
            }
            sleep(10);
        }
        while tufty::buttons().contains(Buttons::A) {
            sleep(10);
        }
        while tufty::get_key().state != tufty::input::KeyState::Idle {}
    }

    let files = match list_media(GAME_PATH, "gb") {
        Ok(files) => files,
        Err(error) => {
            println!("Unable to list ROMs: {:?}", error);
            return;
        }
    };
    let roms = files.iter().map(|path| path.as_str()).collect::<Vec<_>>();

    let selection = {
        // SAFETY: the app core exclusively owns DISPLAY and releases this borrow before emulation.
        let display = unsafe { DISPLAY.as_mut().unwrap() };
        let mut selection_ui = SelectionUi::new(&roms)
            .with_title("GAME BOY")
            .with_hint("/games/gameboy/*.gb");
        match selection_ui.run_selection_ui(display) {
            Ok(maybe_sel) => maybe_sel,
            Err(e) => match e {
                SelectionUiError::SelectionListEmpty => {
                    draw_text_center(
                        display,
                        &format!("No ROMs in / or {}", GAME_PATH),
                        MonoTextStyle::new(&FONT_6X10, Rgb565::RED),
                    )
                    .expect("Display Error");
                    None
                }
                SelectionUiError::DisplayError(_) => panic!("Display Error"),
            },
        }
    };

    let Some(selection) = selection else {
        return;
    };

    let file_name = roms[selection];
    let size = match file_len(&file_name) {
        Ok(size) if size != 0 => size,
        other => {
            println!("Unable to size {}: {:?}", file_name, other);
            return;
        }
    };
    let Ok(buffer) = tufty::PsramBuffer::new(size) else {
        println!("ROM does not fit PSRAM");
        return;
    };
    let mut rom = Rom(buffer);
    let read = read_file(&file_name, 0, &mut rom.0);
    if read != Ok(size) {
        println!("Short read of {}: {:?} of {}", file_name, read, size);
        return;
    }
    println!("Rom size: {}", size);

    // SAFETY: Rom owns these initialized bytes until all synchronous callbacks finish.
    unsafe {
        ROM = core::ptr::slice_from_raw_parts(rom.0.as_ptr(), rom.0.len());
    }

    let mut state: Vec<MaybeUninit<gb_s>> = Vec::new();
    if state.try_reserve_exact(1).is_err() {
        return;
    }
    // SAFETY: capacity for one MaybeUninit context; initialize in place without a large stack copy.
    unsafe {
        state.as_mut_ptr().write_bytes(0, 1);
        state.set_len(1);
    }
    let gb_uninit = &mut state[0];

    // SAFETY: the context storage is valid and callbacks remain available throughout emulation.
    let init_status = unsafe {
        gb_init(
            gb_uninit.as_mut_ptr(),
            Some(gb_rom_read),
            Some(gb_cart_ram_read),
            Some(gb_cart_ram_write),
            Some(gb_error),
            core::ptr::null_mut(),
        )
    };
    println!("gb init status: {}", init_status);
    if init_status != 0 {
        return;
    }

    // SAFETY: gb_init succeeded; zeroed storage initializes fields left untouched by C.
    let gb: &mut gb_s = unsafe { gb_uninit.assume_init_mut() };

    // SAFETY: gb is initialized and the LCD callback lives for the app lifetime.
    unsafe {
        load_save(gb);
        gb_init_lcd(gb, Some(lcd_draw_line));
    };

    // joypad bits are active low; all released
    let joypad: u8 = 0xFF;
    // SAFETY: gb is initialized and exclusively borrowed on this core.
    unsafe { gb_set_joypad(gb, joypad) };

    // skip rendering every other frame when we fall behind
    let mut frame_skip = false;
    let mut next_frame = get_ms() * CLOCK_HZ;

    loop {
        let buttons = tufty::buttons();
        if buttons.intersects(Buttons::HOME) {
            // SAFETY: cartridge RAM remains owned by the app core.
            unsafe { write_save(gb) };
            break;
        }
        let mut joypad = 0xff;
        for (button, mask) in [
            (Buttons::UP, JOYPAD_UP),
            (Buttons::DOWN, JOYPAD_DOWN),
            (Buttons::LEFT, JOYPAD_LEFT),
            (Buttons::RIGHT, JOYPAD_RIGHT),
            (Buttons::A, JOYPAD_A),
            (Buttons::B, JOYPAD_B),
            (Buttons::C | Buttons::START, JOYPAD_START),
            (Buttons::SELECT, JOYPAD_SELECT),
        ] {
            if buttons.intersects(button) {
                joypad &= !(mask as u8);
            }
        }
        // SAFETY: initialized context exclusively owned by the app core.
        unsafe { gb_set_joypad(gb, joypad) };

        // SAFETY: context, ROM, RAM and callbacks remain valid for this frame.
        unsafe { gb_run_frame(gb) };

        // pace to the real gameboy's ~59.7Hz; only skip frames when behind
        next_frame += FRAME_TICKS;
        let deadline = next_frame / CLOCK_HZ;
        let now = get_ms();
        if now < deadline {
            if frame_skip {
                frame_skip = false;
                // SAFETY: gb is initialized and no frame runs concurrently.
                unsafe { gb_set_frame_skip(gb, false) };
            }
            sleep(deadline - now);
        } else {
            if !frame_skip {
                frame_skip = true;
                // SAFETY: gb is initialized and no frame runs concurrently.
                unsafe { gb_set_frame_skip(gb, true) };
            }
            // too far behind to catch up; resync instead of spiralling
            if now - deadline > 10 * FRAME_TICKS / CLOCK_HZ {
                next_frame = now * CLOCK_HZ;
            }
        }
    }
}

unsafe fn load_save(gb: &mut gb_s) {
    let mut buf = [0; 17];

    unsafe {
        gb_get_rom_name(gb, buf.as_mut_ptr());
        let Some(name) = tufty::filename::nul_terminated_name(&buf) else {
            return;
        };

        let save_size = gb_get_save_size(gb);

        if save_size > 0
            && save_size as usize <= RAM_SIZE
            && let Err(error) = read_file(
                &format!("{}/saves/{}.sav", GAME_PATH, name),
                0,
                &mut (&mut *RAM)[..save_size as usize],
            )
        {
            println!("No save loaded: {:?}", error);
        }
    }
}

unsafe fn write_save(gb: &mut gb_s) {
    let mut buf = [0; 17];

    unsafe {
        gb_get_rom_name(gb, buf.as_mut_ptr());
        let Some(name) = tufty::filename::nul_terminated_name(&buf) else {
            return;
        };

        let save_size = gb_get_save_size(gb);

        if save_size > 0
            && save_size as usize <= RAM_SIZE
            && let Err(error) = write_file(
                &format!("{}/saves/{}.sav", GAME_PATH, name),
                0,
                &(&*RAM)[..save_size as usize],
            )
        {
            println!("Save failed: {:?}", error);
        }
    }
}

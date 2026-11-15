#![no_std]
extern crate alloc;
use alloc::{vec, vec::Vec};
use core::mem::MaybeUninit;
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use selection_ui::SelectionUi;
use tufty::{
    Buttons,
    chip8::{Machine, Mode},
    display::Display,
};

pub fn run() {
    let Some(mut display) = Display::take() else {
        return;
    };
    let mut files = tufty::fs::list_media("/games/chip8", "ch8").unwrap_or_default();
    files.extend(tufty::fs::list_media("/games/chip8", "sc8").unwrap_or_default());
    let (rom, mode) = if files.is_empty() {
        (tufty::chip8::DEMO.to_vec(), Mode::Chip8)
    } else {
        let labels: alloc::vec::Vec<&str> = files
            .iter()
            .map(|path| path.rsplit('/').next().unwrap_or(path))
            .collect();
        let Ok(Some(selected)) = SelectionUi::new(&labels)
            .with_title("CHIP-8 / SUPER-CHIP")
            .run_selection_ui(&mut display)
        else {
            return;
        };
        let path = &files[selected];
        let Ok(length) = tufty::fs::file_len(path) else {
            return;
        };
        if length == 0 || length > 3584 {
            return;
        }
        let mut rom = vec![0; length];
        if tufty::fs::read_file(path, 0, &mut rom) != Ok(length) {
            return;
        }
        let mode = if path.to_ascii_lowercase().ends_with(".sc8") {
            Mode::SuperChip
        } else {
            Mode::Chip8
        };
        (rom, mode)
    };
    let mut storage: Vec<MaybeUninit<Machine>> = Vec::new();
    if storage.try_reserve_exact(1).is_err() {
        return;
    }
    // SAFETY: reserved capacity for one MaybeUninit, which permits uninitialized contents.
    unsafe {
        storage.set_len(1);
    }
    let Ok(machine) = Machine::init(
        &mut storage[0],
        &rom,
        mode,
        (tufty::get_ms() as u32).wrapping_add(1),
    ) else {
        return;
    };
    let _ = display.clear(Rgb565::BLACK);
    let mut previous = 0u16;
    let mut tick = tufty::get_ms() * 60;
    let mut frame = vec![0u16; 256 * 128];
    loop {
        let buttons = tufty::buttons();
        if buttons.intersects(Buttons::HOME) || buttons.contains(Buttons::START | Buttons::SELECT) {
            return;
        }
        let mapping = [
            (Buttons::UP, 2),
            (Buttons::DOWN, 8),
            (Buttons::LEFT, 4),
            (Buttons::RIGHT, 6),
            (Buttons::A, 5),
            (Buttons::B, 0),
            (Buttons::C | Buttons::START, 15),
            (Buttons::X, 1),
            (Buttons::Y, 3),
            (Buttons::SELECT, 12),
        ];
        let mut keys = 0;
        for (button, key) in mapping {
            if buttons.intersects(button) {
                keys |= 1 << key;
            }
        }
        if machine.step(keys, keys & !previous).is_err() || machine.halted {
            return;
        }
        previous = keys;
        let now = tufty::get_ms() * 60;
        if now.saturating_sub(tick) >= 1000 {
            let elapsed = (now - tick) / 1000;
            for _ in 0..elapsed.min(255) {
                machine.tick();
            }
            tick += elapsed * 1000;
            if machine.dirty {
                let scale = 256 / machine.width;
                for row in 0..128 {
                    for column in 0..256 {
                        frame[row * 256 + column] = if machine.pixels
                            [(row / scale) * machine.width + column / scale]
                            == 0
                        {
                            0x0821
                        } else {
                            0xff26
                        };
                    }
                }
                display.blit(32, 56, 256, 128, &frame);
                machine.dirty = false;
            }
        }
        tufty::sleep(1);
    }
}

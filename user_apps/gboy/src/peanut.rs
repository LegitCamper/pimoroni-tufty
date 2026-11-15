#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unnecessary_transmutes)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::RAM;
use crate::video::{GBOY_HEIGHT, GBOY_WIDTH, SCALED_HEIGHT, SCALED_WIDTH, scale_line};

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use tufty::{
    display::{SCREEN_HEIGHT, SCREEN_WIDTH},
    println,
};

// centered on the 320x240 panel
const X_OFF: usize = (SCREEN_WIDTH - SCALED_WIDTH) / 2;
const Y_OFF: usize = (SCREEN_HEIGHT - SCALED_HEIGHT) / 2;
const _: () = assert!(X_OFF + SCALED_WIDTH <= SCREEN_WIDTH);
const _: () = assert!(Y_OFF + SCALED_HEIGHT <= SCREEN_HEIGHT);

/// Rom bytes, cached as a slice so the per-byte read callback is a single
/// bounds compare instead of an `Option` unwrap + panic path.
pub static mut ROM: *const [u8] = &[];

pub unsafe extern "C" fn gb_rom_read(_gb: *mut gb_s, addr: u32) -> u8 {
    unsafe { (&*ROM).get(addr as usize).copied().unwrap_or(0xFF) }
}

pub unsafe extern "C" fn gb_cart_ram_read(_gb: *mut gb_s, addr: u32) -> u8 {
    unsafe { (&*RAM).get(addr as usize).copied().unwrap_or(0xFF) }
}

pub unsafe extern "C" fn gb_cart_ram_write(_gb: *mut gb_s, addr: u32, val: u8) {
    if let Some(slot) = unsafe { (&mut *RAM).get_mut(addr as usize) } {
        *slot = val;
    }
}

pub unsafe extern "C" fn gb_error(_gb: *mut gb_s, err: gb_error_e, addr: u16) {
    let e = match err {
        0 => "UNKNOWN ERROR",
        1 => "INVALID OPCODE",
        2 => "INVALID READ",
        3 => "INVALID WRITE",
        4 => "HALT FOREVER",
        5 => "INVALID MAX",
        _ => "BAD ERROR CODE",
    };

    println!("PeanutGB error: {}, addr: {}", e, addr);
}

pub unsafe extern "C" fn lcd_draw_line(_gb: *mut gb_s, pixels: *const u8, line: u8) {
    if line as usize >= GBOY_HEIGHT {
        return;
    }

    // SAFETY: Peanut-GB always hands us a full LCD_WIDTH scanline
    let src = unsafe { &*pixels.cast::<[u8; GBOY_WIDTH]>() };

    let mut rows = [0_u16; SCALED_WIDTH * 2];
    scale_line(src, &mut rows);

    // SAFETY: the app core owns DISPLAY; the synchronous blit copies both complete rows.
    unsafe {
        crate::DISPLAY.as_mut().unwrap().blit(
            X_OFF as u16,
            (Y_OFF + line as usize * SCALED_HEIGHT / GBOY_HEIGHT) as u16,
            SCALED_WIDTH as u16,
            (((line as usize + 1) * SCALED_HEIGHT / GBOY_HEIGHT)
                - line as usize * SCALED_HEIGHT / GBOY_HEIGHT) as u16,
            &rows,
        )
    };
}

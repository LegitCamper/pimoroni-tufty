use strum::IntoEnumIterator;

pub struct App {
    pub title: &'static str,
    pub icon: &'static [u8],
    pub entry: tufty_sys::EntryFn,
}

pub static APPS: &[App] = &[
    App {
        title: "PICO-8 Mini",
        icon: include_bytes!("../../assets/pico8.png"),
        entry: tufty_love::run_pico8,
    },
    App {
        title: "Lua / LOVE Mini",
        icon: include_bytes!("../../assets/love.png"),
        entry: tufty_love::run,
    },
    App {
        title: "NES",
        icon: include_bytes!("../../assets/nes.png"),
        entry: consoles::nes,
    },
    App {
        title: "SNES",
        icon: include_bytes!("../../assets/snes.png"),
        entry: consoles::snes,
    },
    App {
        title: "Game Boy Color",
        icon: include_bytes!("../../assets/gbc.png"),
        entry: consoles::gbc,
    },
    App {
        title: "Game Boy Advance",
        icon: include_bytes!("../../assets/gba.png"),
        entry: consoles::gba,
    },
    App {
        title: "Game Boy",
        icon: include_bytes!("../../assets/gboy.png"),
        entry: gboy::run,
    },
    App {
        title: "CHIP-8 / SCHIP",
        icon: include_bytes!("../../assets/chip8.png"),
        entry: chip8::run,
    },
    App {
        title: "Snake",
        icon: include_bytes!("../../assets/snake.png"),
        entry: snake::main,
    },
];

pub fn init() {
    for call in tufty_sys::SyscallTable::iter() {
        // SAFETY: initialized once on core0 before core1 starts; never changed during execution.
        unsafe {
            tufty_sys::SYS_CALL_TABLE[call as usize] = crate::syscalls::pointer(call);
        }
    }
}

use strum::IntoEnumIterator;

pub struct App {
    pub title: &'static str,
    pub icon: &'static [u8],
    pub entry: tufty_sys::EntryFn,
}

pub static APPS: &[App] = &[
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

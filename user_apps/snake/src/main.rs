#![no_std]
#![no_main]

#[cfg(feature = "builtin")]
compile_error!("Build standalone apps separately from the kernel; use just apps");

tufty::app_metadata!("Snake", "../../../assets/snake.png");

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        tufty::sleep(1000);
    }
}

#[unsafe(no_mangle)]
pub fn _start() {
    snake::main();
}

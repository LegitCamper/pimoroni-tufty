#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]
#![allow(static_mut_refs)]

extern crate alloc;
mod audio;
mod builtin;
mod display;
mod elf;
mod heap;
mod input;
mod power;
mod psram;
mod runtime;
mod storage;
mod syscalls;
mod ui;
mod user_memory;
mod utils;
#[cfg(feature = "wifi")]
mod wireless;

use core::sync::atomic::{AtomicBool, Ordering};
use embassy_executor::{Executor, Spawner};
use embassy_rp::{
    gpio::{Input, Level, Output, Pull},
    i2c::{self, I2c},
    multicore::{Stack, spawn_core1},
    peripherals::{I2C0, PIO0, PIO1},
    pio::{self, Config, FifoJoin, Pio, ShiftConfig, ShiftDirection, program::pio_asm},
    pwm::Pwm,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Instant, Timer};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

embassy_rp::bind_interrupts!(struct Irqs {
    I2C0_IRQ => i2c::InterruptHandler<I2C0>;
    PIO1_IRQ_0 => pio::InterruptHandler<PIO1>;
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
});

static mut CORE1_STACK: Stack<32768> = Stack::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();
#[global_allocator]
static ALLOCATOR: runtime::Allocator = runtime::Allocator;
pub static APP_RUNNING: AtomicBool = AtomicBool::new(false);
pub static BINARY_CH: Channel<
    CriticalSectionRawMutex,
    (
        tufty_sys::EntryFn,
        Option<elf::ProgramImage>,
        tufty_sys::memory::Region,
    ),
    1,
> = Channel::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let peripherals = embassy_rp::init(Default::default());
    let backlight = Pwm::new_output_a(
        peripherals.PWM_SLICE5,
        peripherals.PIN_26,
        display::backlight_config(),
    );
    power::check_reset(peripherals.PIN_14).await;
    runtime::init();
    builtin::init();
    let _power = Output::new(peripherals.PIN_41, Level::High);
    Timer::after_millis(50).await;
    let size = psram::init_psram_qmi(&embassy_rp::pac::QMI, &embassy_rp::pac::XIP_CTRL);
    assert_eq!(size, 8 * 1024 * 1024, "Tufty requires 8 MiB PSRAM");
    heap::init_qmi_psram_heap(size);
    // SAFETY: PSRAM QMI setup finishes before core1 can fetch any XIP code.
    spawn_core1(
        peripherals.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        || {
            EXECUTOR1
                .init(Executor::new())
                .run(|spawner| spawner.spawn(userland_task()).unwrap());
        },
    );
    let mut config = i2c::Config::default();
    config.frequency = 100_000;
    let bus = I2c::new_async(
        peripherals.I2C0,
        peripherals.PIN_5,
        peripherals.PIN_4,
        Irqs,
        config,
    );
    spawner
        .spawn(input::input_task(
            [
                Input::new(peripherals.PIN_11, Pull::Up),
                Input::new(peripherals.PIN_6, Pull::Up),
                Input::new(peripherals.PIN_7, Pull::Up),
                Input::new(peripherals.PIN_9, Pull::Up),
                Input::new(peripherals.PIN_10, Pull::Up),
                Input::new(peripherals.PIN_22, Pull::Up),
            ],
            bus,
        ))
        .unwrap();
    let Pio {
        mut common,
        mut sm0,
        ..
    } = Pio::new(peripherals.PIO1, Irqs);
    let program = pio_asm!(
        ".side_set 1",
        ".wrap_target",
        "out pins, 8 side 0",
        "nop side 1",
        ".wrap"
    );
    let program = common.load_program(&program.program);
    let write = common.make_pio_pin(peripherals.PIN_30);
    let data = [
        common.make_pio_pin(peripherals.PIN_32),
        common.make_pio_pin(peripherals.PIN_33),
        common.make_pio_pin(peripherals.PIN_34),
        common.make_pio_pin(peripherals.PIN_35),
        common.make_pio_pin(peripherals.PIN_36),
        common.make_pio_pin(peripherals.PIN_37),
        common.make_pio_pin(peripherals.PIN_38),
        common.make_pio_pin(peripherals.PIN_39),
    ];
    let refs = data.each_ref();
    let mut config = Config::default();
    config.use_program(&program, &[&write]);
    config.set_out_pins(&refs);
    config.fifo_join = FifoJoin::TxOnly;
    config.shift_out = ShiftConfig {
        auto_fill: true,
        threshold: 8,
        direction: ShiftDirection::Left,
    };
    config.clock_divider = fixed::types::U24F8::from_num(4);
    sm0.set_config(&config);
    // ponytail: PIO1/SM0 only; use set_pin_dirs once Embassy handles GPIOBASE there.
    let pinctrl = embassy_rp::pac::PIO1.sm(0).pinctrl();
    let saved_pinctrl = pinctrl.read();
    for pin in refs.into_iter().chain(core::iter::once(&write)) {
        pinctrl.modify(|control| {
            control.set_set_base(pin.pin() - 16);
            control.set_set_count(1);
        });
        // SAFETY: SM0 is disabled; SET PINDIRS only enables our owned LCD outputs.
        unsafe { sm0.exec_instr(0xe081) };
    }
    pinctrl.write_value(saved_pinctrl);
    assert_eq!(embassy_rp::pac::PIO1.dbg_padoe().read(), 0x00ff_4000);
    sm0.set_enable(true);
    spawner
        .spawn(display::display_task(display::Panel {
            common,
            sm: sm0,
            dma: peripherals.DMA_CH0,
            cs: Output::new(peripherals.PIN_27, Level::High),
            dc: Output::new(peripherals.PIN_28, Level::High),
            read: Output::new(peripherals.PIN_31, Level::High),
            backlight,
        }))
        .unwrap();
    #[cfg(feature = "wifi")]
    wireless::start(
        spawner,
        peripherals.PIO0,
        peripherals.DMA_CH1,
        peripherals.PIN_23,
        peripherals.PIN_24,
        peripherals.PIN_25,
        peripherals.PIN_29,
    )
    .await;
    #[cfg(not(feature = "wifi"))]
    let _radio_off = Output::new(peripherals.PIN_23, Level::Low);
    ui::run().await;
}

#[embassy_executor::task]
async fn userland_task() {
    loop {
        let (entry, image, region) = BINARY_CH.receive().await;
        // SAFETY: timestamp has exactly one writer and reader, both on core1.
        unsafe {
            syscalls::MS_SINCE_LAUNCH = Some(Instant::now());
        }
        let stack = core::ptr::addr_of!(CORE1_STACK) as usize + 256;
        let stack = tufty_sys::memory::Region {
            start: stack,
            len: (cortex_m::register::msp::read() as usize).saturating_sub(stack),
        };
        if image.is_some() {
            user_memory::begin(region, stack);
        } else {
            user_memory::begin_builtin(stack);
        }
        entry();
        user_memory::finish();
        drop(image);
        assert_eq!(runtime::APP_HEAP.used(), 0, "app SRAM leaked");
        while syscalls::KEY_CACHE.try_receive().is_ok() {}
        APP_RUNNING.store(false, Ordering::Release);
    }
}

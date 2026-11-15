use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use embassy_rp::{
    gpio::Input,
    i2c::{Async, I2c},
    peripherals::I2C0,
};
use embassy_time::{Duration, Timer, with_timeout};
use tufty_sys::{
    buttons::{Buttons, Debouncer, gamepad_buttons},
    input::{KeyCode, KeyEvent, KeyState},
};

pub static BUTTONS: AtomicU32 = AtomicU32::new(0);
pub static GAMEPAD: AtomicBool = AtomicBool::new(false);
pub static LEDS: AtomicU8 = AtomicU8::new(0);

#[embassy_executor::task]
pub async fn input_task(pins: [Input<'static>; 6], mut bus: I2c<'static, I2C0, Async>) {
    let front = [
        Buttons::UP,
        Buttons::DOWN,
        Buttons::A,
        Buttons::B,
        Buttons::C,
        Buttons::HOME,
    ];
    let mapping = [
        (Buttons::UP, KeyCode::Up),
        (Buttons::DOWN, KeyCode::Down),
        (Buttons::LEFT, KeyCode::Left),
        (Buttons::RIGHT, KeyCode::Right),
        (Buttons::A, KeyCode::Enter),
        (Buttons::B, KeyCode::Backspace),
        (Buttons::C | Buttons::START, KeyCode::Tab),
        (Buttons::SELECT, KeyCode::Del),
        (Buttons::HOME | Buttons::X, KeyCode::Esc),
    ];
    let mut debounce = Debouncer::default();
    let mut previous = Buttons::empty();
    let mut address = None;
    let mut retry = 0u16;
    let mut sent_leds = u8::MAX;
    loop {
        if address.is_none() && retry == 0 {
            for candidate in [0x21u8, 0x23, 0x25, 0x27] {
                let configured = with_timeout(Duration::from_millis(20), async {
                    bus.write_async(candidate, [0x06, 0x3f, 0xf9]).await?;
                    bus.write_async(candidate, [0x04, 0x3f, 0xf8]).await?;
                    bus.write_async(candidate, [0x02, 0xc0, 0x06]).await
                })
                .await;
                if matches!(configured, Ok(Ok(()))) {
                    address = Some(candidate);
                    sent_leds = u8::MAX;
                    break;
                }
            }
            retry = 200;
        }
        retry = retry.saturating_sub(1);
        let mut raw = Buttons::empty();
        for (pin, button) in pins.iter().zip(front) {
            if pin.is_low() {
                raw |= button;
            }
        }
        if let Some(device) = address {
            let mut bytes = [0; 2];
            if matches!(
                with_timeout(
                    Duration::from_millis(10),
                    bus.write_read_async(device, [0x00], &mut bytes)
                )
                .await,
                Ok(Ok(()))
            ) {
                raw |= gamepad_buttons(u16::from_le_bytes(bytes));
                let desired = LEDS.load(Ordering::Relaxed);
                if desired != sent_leds {
                    let mut output = 0u16;
                    for (index, pin) in [6, 7, 9, 10].into_iter().enumerate() {
                        if desired & (1 << index) == 0 {
                            output |= 1 << pin;
                        }
                    }
                    let bytes = output.to_le_bytes();
                    if matches!(
                        with_timeout(
                            Duration::from_millis(10),
                            bus.write_async(device, [0x02, bytes[0], bytes[1]])
                        )
                        .await,
                        Ok(Ok(()))
                    ) {
                        sent_leds = desired;
                    }
                }
            } else {
                address = None;
            }
        }
        GAMEPAD.store(address.is_some(), Ordering::Release);
        let current = debounce.sample(raw);
        BUTTONS.store(current.bits(), Ordering::Release);
        for (buttons, key) in mapping {
            let was_down = previous.intersects(buttons);
            let is_down = current.intersects(buttons);
            if was_down != is_down {
                let event = KeyEvent {
                    key,
                    state: if is_down {
                        KeyState::Pressed
                    } else {
                        KeyState::Released
                    },
                    mods: Default::default(),
                };
                let _ = crate::syscalls::KEY_CACHE.try_send(event);
            }
        }
        previous = current;
        Timer::after_millis(5).await;
    }
}

use core::convert::Infallible;
use core::sync::atomic::{AtomicU8, Ordering};
use embassy_rp::{
    Peri,
    gpio::Output,
    peripherals::{DMA_CH0, PIO1},
    pio::{Common, StateMachine},
    pwm::{Config, Pwm},
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::Timer;
use embedded_graphics::{Pixel, pixelcolor::Rgb565, prelude::*};

pub const SCREEN_WIDTH: usize = 320;
pub const SCREEN_HEIGHT: usize = 240;
pub static FRAMEBUFFER: Mutex<CriticalSectionRawMutex, Option<FrameBuffer>> = Mutex::new(None);
static mut PIXELS: [u16; SCREEN_WIDTH * SCREEN_HEIGHT] = [0; SCREEN_WIDTH * SCREEN_HEIGHT];
// ponytail: brightness is RAM-only and tap-stepped; add settings/repeat when needed.
pub static BRIGHTNESS: AtomicU8 = AtomicU8::new(100);

pub fn backlight_config() -> Config {
    let mut config = Config::default();
    config.top = 999;
    config.divider = 8.into();
    config.compare_a = 0;
    config
}

pub struct FrameBuffer {
    pixels: &'static mut [u16],
    dirty: bool,
}
impl OriginDimensions for FrameBuffer {
    fn size(&self) -> Size {
        Size::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)
    }
}
impl DrawTarget for FrameBuffer {
    type Color = Rgb565;
    type Error = Infallible;
    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Infallible> {
        for Pixel(point, color) in pixels {
            if point.x >= 0
                && point.y >= 0
                && point.x < SCREEN_WIDTH as i32
                && point.y < SCREEN_HEIGHT as i32
            {
                self.pixels[point.y as usize * SCREEN_WIDTH + point.x as usize] =
                    color.into_storage();
                self.dirty = true;
            }
        }
        Ok(())
    }
    fn clear(&mut self, color: Rgb565) -> Result<(), Infallible> {
        self.pixels.fill(color.into_storage());
        self.dirty = true;
        Ok(())
    }
}

pub struct Panel {
    pub common: Common<'static, PIO1>,
    pub sm: StateMachine<'static, PIO1, 0>,
    pub dma: Peri<'static, DMA_CH0>,
    pub cs: Output<'static>,
    pub dc: Output<'static>,
    pub read: Output<'static>,
    pub backlight: Pwm<'static>,
}
impl Panel {
    async fn write(&mut self, bytes: &[u8]) {
        self.sm
            .tx()
            .dma_push(self.dma.reborrow(), bytes, false)
            .await;
        self.sm.tx().stalled();
        while !self.sm.tx().stalled() {
            embassy_futures::yield_now().await;
        }
    }
    async fn command(&mut self, command: u8, data: &[u8]) {
        self.cs.set_low();
        self.dc.set_low();
        self.write(&[command]).await;
        if !data.is_empty() {
            self.dc.set_high();
            self.write(data).await;
        }
        self.cs.set_high();
    }
}

#[embassy_executor::task]
pub async fn display_task(mut panel: Panel) {
    let _ = (&panel.common, &panel.read);
    panel.command(0x01, &[]).await;
    Timer::after_millis(150).await;
    for (command, data) in [
        (0x3a, &[0x05][..]),
        (0xb2, &[0x0c, 0x0c, 0, 0x33, 0x33]),
        (0xc0, &[0x2c]),
        (0xc2, &[1]),
        (0xc3, &[0x0f]),
        (0xc4, &[0x20]),
        (0xd0, &[0xa4, 0xa1]),
        (0xc6, &[0x0f]),
        (0xb0, &[0, 0xc0]),
        (0xb7, &[0x35]),
        (0xbb, &[0x1b]),
        (
            0xe0,
            &[
                0xf0, 0, 6, 4, 5, 5, 0x31, 0x44, 0x48, 0x36, 0x12, 0x12, 0x2b, 0x34,
            ],
        ),
        (
            0xe1,
            &[
                0xf0, 0x0b, 0x0f, 0x0f, 0x0d, 0x26, 0x31, 0x43, 0x47, 0x38, 0x14, 0x14, 0x2c, 0x32,
            ],
        ),
        (0x21, &[]),
        (0x11, &[]),
    ] {
        panel.command(command, data).await;
    }
    Timer::after_millis(100).await;
    panel.command(0x36, &[0x90]).await;
    panel.command(0x2a, &[0, 0, 0, 239]).await;
    panel.command(0x2b, &[0, 0, 1, 63]).await;
    let mut line = [0u8; SCREEN_HEIGHT * 2];
    panel.command(0x2c, &[]).await;
    panel.cs.set_low();
    panel.dc.set_high();
    for _ in 0..SCREEN_WIDTH {
        panel.write(&line).await;
    }
    panel.cs.set_high();
    panel.command(0x29, &[]).await;
    Timer::after_millis(20).await;
    let mut backlight = backlight_config();
    backlight.compare_a = u16::from(BRIGHTNESS.load(Ordering::Relaxed)) * 10;
    panel.backlight.set_config(&backlight);
    // SAFETY: this task initializes the static buffer once; mutex guards every subsequent access.
    *FRAMEBUFFER.lock().await = Some(FrameBuffer {
        pixels: unsafe { &mut *core::ptr::addr_of_mut!(PIXELS) },
        dirty: false,
    });
    loop {
        let duty = u16::from(BRIGHTNESS.load(Ordering::Relaxed)) * 10;
        if backlight.compare_a != duty {
            backlight.compare_a = duty;
            panel.backlight.set_config(&backlight);
        }
        {
            let mut guard = FRAMEBUFFER.lock().await;
            if let Some(frame) = guard.as_mut().filter(|frame| frame.dirty) {
                panel.command(0x2c, &[]).await;
                panel.cs.set_low();
                panel.dc.set_high();
                for column in 0..SCREEN_WIDTH {
                    for row in 0..SCREEN_HEIGHT {
                        line[row * 2..row * 2 + 2].copy_from_slice(
                            &frame.pixels[row * SCREEN_WIDTH + column].to_be_bytes(),
                        );
                    }
                    panel.write(&line).await;
                }
                panel.cs.set_high();
                frame.dirty = false;
            }
        }
        Timer::after_millis(16).await;
    }
}

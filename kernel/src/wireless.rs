use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, Ordering};
use cyw43_pio::PioSpi;
use embassy_executor::Spawner;
use embassy_rp::{
    Peri,
    gpio::{Level, Output},
    peripherals::{DMA_CH1, PIN_23, PIN_24, PIN_25, PIN_29, PIO0},
    pio::Pio,
};
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    signal::Signal,
};
use embassy_time::{Duration, with_timeout};
use static_cell::StaticCell;
use tufty_sys::{
    SysError,
    wireless::{MAX_NETWORKS, WifiNetwork},
};

pub static READY: AtomicBool = AtomicBool::new(false);
static STATE: StaticCell<cyw43::State> = StaticCell::new();
pub static SCANNING: AtomicBool = AtomicBool::new(false);
static FAILED: AtomicBool = AtomicBool::new(false);
static SCAN: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static NETWORKS: Mutex<CriticalSectionRawMutex, RefCell<heapless::Vec<WifiNetwork, MAX_NETWORKS>>> =
    Mutex::new(RefCell::new(heapless::Vec::new()));

pub fn scan() -> Result<(), SysError> {
    if !READY.load(Ordering::Acquire) {
        return Err(SysError::Io);
    }
    if SCANNING.swap(true, Ordering::AcqRel) {
        return Err(SysError::Busy);
    }
    SCAN.signal(());
    Ok(())
}
pub fn network(index: usize) -> Result<WifiNetwork, SysError> {
    if !READY.load(Ordering::Acquire) || FAILED.load(Ordering::Acquire) {
        return Err(SysError::Io);
    }
    if SCANNING.load(Ordering::Acquire) {
        return Err(SysError::Busy);
    }
    NETWORKS.lock(|networks| {
        networks
            .borrow()
            .get(index)
            .copied()
            .ok_or(SysError::NotFound)
    })
}

#[embassy_executor::task]
async fn control_task(mut control: cyw43::Control<'static>) {
    loop {
        SCAN.wait().await;
        NETWORKS.lock(|networks| networks.borrow_mut().clear());
        FAILED.store(false, Ordering::Release);
        let result = with_timeout(Duration::from_secs(15), async {
            let mut scanner = control.scan(Default::default()).await;
            while let Some(found) = scanner.next().await {
                NETWORKS.lock(|networks| {
                    let mut networks = networks.borrow_mut();
                    if found.ssid_len <= 32
                        && !networks.iter().any(|network| network.bssid == found.bssid)
                    {
                        let _ = networks.push(WifiNetwork {
                            ssid: found.ssid,
                            ssid_len: found.ssid_len,
                            channel: (found.chanspec & 255) as u8,
                            rssi: found.rssi,
                            bssid: found.bssid,
                            reserved: [0; 2],
                        });
                    }
                });
            }
        })
        .await;
        FAILED.store(result.is_err(), Ordering::Release);
        SCANNING.store(false, Ordering::Release);
    }
}

#[embassy_executor::task]
async fn runner(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH1>>,
) -> ! {
    runner.run().await
}

pub async fn start(
    spawner: Spawner,
    pio: Peri<'static, PIO0>,
    dma: Peri<'static, DMA_CH1>,
    power: Peri<'static, PIN_23>,
    data: Peri<'static, PIN_24>,
    cs: Peri<'static, PIN_25>,
    clock: Peri<'static, PIN_29>,
) {
    let pwr = Output::new(power, Level::Low);
    let cs = Output::new(cs, Level::High);
    let mut pio = Pio::new(pio, crate::Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        cyw43_pio::RM2_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        data,
        clock,
        dma,
    );
    let state = STATE.init(cyw43::State::new());
    let setup = cyw43::new(
        state,
        pwr,
        spi,
        include_bytes!("../../firmware/43439A0.bin"),
    );
    let Ok((_network_device, mut control, task)) =
        with_timeout(Duration::from_secs(10), setup).await
    else {
        return;
    };
    spawner.spawn(runner(task)).unwrap();
    if with_timeout(
        Duration::from_secs(10),
        control.init(include_bytes!("../../firmware/43439A0_clm.bin")),
    )
    .await
    .is_err()
    {
        return;
    }
    if with_timeout(
        Duration::from_secs(2),
        control.set_power_management(cyw43::PowerManagementMode::PowerSave),
    )
    .await
    .is_err()
    {
        return;
    }
    spawner.spawn(control_task(control)).unwrap();
    READY.store(true, Ordering::Release);
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct WifiNetwork {
    pub ssid: [u8; 32],
    pub ssid_len: u8,
    pub channel: u8,
    pub rssi: i16,
    pub bssid: [u8; 6],
    pub reserved: [u8; 2],
}
const _: () = assert!(core::mem::size_of::<WifiNetwork>() == 44);

pub const GAMEPAD_CONNECTED: u32 = 1;
pub const WIFI_READY: u32 = 2;
pub const WIFI_SCANNING: u32 = 4;
pub const STORAGE_READ_ONLY: u32 = 8;
pub const MAX_NETWORKS: usize = 16;

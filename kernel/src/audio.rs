use tufty_sys::SysError;

pub fn ready() -> bool {
    false
}
pub fn configure(_: u32) -> Result<(), SysError> {
    Err(SysError::Unsupported)
}
pub fn submit(_: &[u8]) -> Result<(), SysError> {
    Err(SysError::Unsupported)
}

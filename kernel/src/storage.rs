use alloc::{string::String, vec::Vec};
use embassy_sync::lazy_lock::LazyLock;
use tufty_sys::{SysError, pack::Pack};

static PACK: LazyLock<Result<Pack<'static>, SysError>> = LazyLock::new(|| {
    // SAFETY: reserved, read-only flash region; firmware never writes XIP while running.
    let bytes = unsafe {
        core::slice::from_raw_parts(
            (0x1000_0000 + tufty_sys::pack::FLASH_OFFSET) as *const u8,
            tufty_sys::pack::MAX_SIZE,
        )
    };
    Pack::parse(bytes)
});

pub fn pack() -> Result<Pack<'static>, SysError> {
    *PACK.get()
}

pub fn file(path: &str) -> Result<&'static [u8], SysError> {
    pack()?.file(path)
}

pub fn list(path: &str) -> Result<Vec<String>, SysError> {
    if tufty_sys::split_path(path).is_none() {
        return Err(SysError::InvalidArgument);
    }
    let pack = pack()?;
    let mut names = Vec::new();
    for index in 0..pack.len() {
        let (name, _) = pack.entry(index)?;
        let rest = if path == "/" {
            name.strip_prefix('/')
        } else {
            name.strip_prefix(path)
                .and_then(|rest| rest.strip_prefix('/'))
        };
        if let Some(rest) = rest {
            let child = rest.split('/').next().ok_or(SysError::Io)?;
            if !names.iter().any(|name| name == child) {
                names.try_reserve(1).map_err(|_| SysError::NoMemory)?;
                let mut name = String::new();
                name.try_reserve_exact(child.len())
                    .map_err(|_| SysError::NoMemory)?;
                name.push_str(child);
                names.push(name);
            }
        }
    }
    if names.is_empty() && path != "/" {
        return Err(SysError::NotFound);
    }
    names.sort_unstable();
    Ok(names)
}

pub struct File<'a> {
    data: &'a [u8],
    position: usize,
}
impl<'a> File<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }
    pub fn length(&self) -> u32 {
        self.data.len() as u32
    }
    pub fn seek_from_start(&mut self, offset: u32) -> Result<(), SysError> {
        if offset as usize > self.data.len() {
            return Err(SysError::InvalidArgument);
        }
        self.position = offset as usize;
        Ok(())
    }
    pub fn read(&mut self, output: &mut [u8]) -> Result<usize, SysError> {
        let count = output.len().min(self.data.len() - self.position);
        output[..count].copy_from_slice(&self.data[self.position..self.position + count]);
        self.position += count;
        Ok(count)
    }
}

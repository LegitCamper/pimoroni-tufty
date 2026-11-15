use crate::SysError;
use core::{
    alloc::Layout,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

/// App-owned bytes in PSRAM. Do not place exclusive atomics in this storage.
pub struct PsramBuffer {
    pointer: NonNull<u8>,
    len: usize,
}

impl PsramBuffer {
    pub fn new(len: usize) -> Result<Self, SysError> {
        let layout = Layout::array::<u8>(len).map_err(|_| SysError::InvalidArgument)?;
        let pointer = if len == 0 {
            NonNull::dangling()
        } else {
            // SAFETY: validated nonempty layout; runtime initializes the syscall table.
            let pointer = unsafe { tufty_sys::alloc(layout.into()) };
            let pointer = NonNull::new(pointer).ok_or(SysError::NoMemory)?;
            // SAFETY: exclusively owned allocation has exactly len bytes.
            unsafe {
                pointer.as_ptr().write_bytes(0, len);
            }
            pointer
        };
        Ok(Self { pointer, len })
    }
}

impl Deref for PsramBuffer {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        // SAFETY: allocation is initialized and owned; dangling is valid for an empty slice.
        unsafe { core::slice::from_raw_parts(self.pointer.as_ptr(), self.len) }
    }
}

impl DerefMut for PsramBuffer {
    fn deref_mut(&mut self) -> &mut [u8] {
        // SAFETY: exclusive borrow of the complete owned allocation.
        unsafe { core::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.len) }
    }
}

impl Drop for PsramBuffer {
    fn drop(&mut self) {
        if self.len != 0 {
            // SAFETY: same live allocation and layout accepted by new; released exactly once.
            unsafe {
                tufty_sys::dealloc(
                    self.pointer.as_ptr(),
                    Layout::from_size_align_unchecked(self.len, 1).into(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};
    static LIVE: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "C" fn allocate(layout: tufty_sys::CLayout) -> *mut u8 {
        let layout = Layout::try_from(layout).unwrap();
        if layout.size() > 256 {
            return core::ptr::null_mut();
        }
        LIVE.fetch_add(1, Ordering::Relaxed);
        // SAFETY: test forwards a checked nonempty layout to the host allocator.
        unsafe { std::alloc::alloc(layout) }
    }
    unsafe extern "C" fn free(pointer: *mut u8, layout: tufty_sys::CLayout) {
        LIVE.fetch_sub(1, Ordering::Relaxed);
        // SAFETY: buffer returns the exact allocation and layout once.
        unsafe {
            std::alloc::dealloc(pointer, layout.try_into().unwrap());
        }
    }

    #[test]
    fn buffer_validates_size_initializes_and_releases_on_each_launch() {
        // SAFETY: only this test calls these two syscall slots in the host test process.
        unsafe {
            tufty_sys::SYS_CALL_TABLE[tufty_sys::SyscallTable::Alloc as usize] =
                allocate as *const () as usize;
            tufty_sys::SYS_CALL_TABLE[tufty_sys::SyscallTable::Dealloc as usize] =
                free as *const () as usize;
        }
        assert!(PsramBuffer::new(usize::MAX).is_err());
        assert!(matches!(PsramBuffer::new(257), Err(SysError::NoMemory)));
        assert!(PsramBuffer::new(0).unwrap().is_empty());
        for _ in 0..3 {
            let mut buffer = PsramBuffer::new(128).unwrap();
            assert!(buffer.iter().all(|&byte| byte == 0));
            buffer[127] = 42;
            assert_eq!(buffer[127], 42);
            assert_eq!(LIVE.load(Ordering::Relaxed), 1);
            drop(buffer);
            assert_eq!(LIVE.load(Ordering::Relaxed), 0);
        }
    }
}

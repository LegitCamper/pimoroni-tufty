use crate::{audio, display::FRAMEBUFFER, user_memory, utils::wait};
use core::{
    alloc::Layout,
    ffi::c_char,
    ptr, slice,
    sync::atomic::{AtomicI32, Ordering},
};
use embassy_rp::clocks::RoscRng;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Duration, Instant, Timer};
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Point, Size},
    pixelcolor::{Rgb565, raw::RawU16},
    primitives::Rectangle,
};
use tufty_sys::{
    Alloc, AudioBufferReady, Blit, CLayout, CPixel, Dealloc, DrawIter, FileLen, FillRect, GenRand,
    GetLastError, GetMs, ListDir, Print, ReadFile, ReconfigureAudioSampleRate, RngRequest,
    SendAudioBuffer, SleepMs, SysError, WriteFile, input::*, memory::Region, split_path,
};

static LAST_ERROR: AtomicI32 = AtomicI32::new(0);

pub fn pointer(call: tufty_sys::SyscallTable) -> usize {
    use tufty_sys::SyscallTable;
    match call {
        SyscallTable::Alloc => alloc as usize,
        SyscallTable::Dealloc => dealloc as usize,
        SyscallTable::PrintString => print as usize,
        SyscallTable::SleepMs => sleep as usize,
        SyscallTable::GetMs => get_ms as usize,
        SyscallTable::DrawIter => draw_iter as usize,
        SyscallTable::GetKey => get_key as usize,
        SyscallTable::GenRand => gen_rand as usize,
        SyscallTable::ListDir => list_dir as usize,
        SyscallTable::ReadFile => read_file as usize,
        SyscallTable::WriteFile => write_file as usize,
        SyscallTable::FileLen => file_len as usize,
        SyscallTable::ReconfigureAudioSampleRate => reconfigure_audio_sample_rate as usize,
        SyscallTable::AudioBufferReady => audio_buffer_ready as usize,
        SyscallTable::SendAudioBuffer => send_audio_buffer as usize,
        SyscallTable::FillRect => fill_rect as usize,
        SyscallTable::Blit => blit as usize,
        SyscallTable::GetLastError => get_last_error as usize,
        SyscallTable::ReadButtons => read_buttons as usize,
        SyscallTable::DeviceStatus => device_status as usize,
        SyscallTable::GamepadLeds => gamepad_leds as usize,
        SyscallTable::WifiScan => wifi_scan as usize,
        SyscallTable::WifiNetwork => wifi_network as usize,
    }
}
pub extern "C" fn read_buttons() -> u32 {
    crate::input::BUTTONS.load(Ordering::Acquire)
}
pub extern "C" fn device_status() -> u32 {
    let status = u32::from(crate::input::GAMEPAD.load(Ordering::Acquire))
        | tufty_sys::wireless::STORAGE_READ_ONLY;
    #[cfg(feature = "wifi")]
    let status = status
        | (u32::from(crate::wireless::READY.load(Ordering::Acquire)) << 1)
        | (u32::from(crate::wireless::SCANNING.load(Ordering::Acquire)) << 2);
    status
}
pub extern "C" fn gamepad_leds(mask: u32) -> i32 {
    if mask > 15 {
        return SysError::InvalidArgument as i32;
    }
    if !crate::input::GAMEPAD.load(Ordering::Acquire) {
        return SysError::NotFound as i32;
    }
    crate::input::LEDS.store(mask as u8, Ordering::Release);
    0
}
pub extern "C" fn wifi_scan() -> i32 {
    #[cfg(feature = "wifi")]
    {
        crate::wireless::scan().map_or_else(|error| error as i32, |()| 0)
    }
    #[cfg(not(feature = "wifi"))]
    {
        SysError::Unsupported as i32
    }
}
pub unsafe extern "C" fn wifi_network(
    index: u32,
    output: *mut tufty_sys::wireless::WifiNetwork,
) -> i32 {
    if !valid_buffer(output, 1) || index as usize >= tufty_sys::wireless::MAX_NETWORKS {
        return SysError::InvalidArgument as i32;
    }
    #[cfg(feature = "wifi")]
    {
        match crate::wireless::network(index as usize) {
            Ok(network) => {
                // SAFETY: entire aligned output lies in live app-owned writable memory.
                unsafe {
                    output.write(network);
                }
                0
            }
            Err(error) => error as i32,
        }
    }
    #[cfg(not(feature = "wifi"))]
    {
        SysError::Unsupported as i32
    }
}
fn report<T>(result: Result<T, SysError>, fallback: T) -> T {
    match result {
        Ok(value) => {
            LAST_ERROR.store(0, Ordering::Release);
            value
        }
        Err(error) => {
            #[cfg(feature = "defmt")]
            defmt::warn!("Syscall rejected: {}", error as i32);
            LAST_ERROR.store(error as i32, Ordering::Release);
            fallback
        }
    }
}
const _: GetLastError = get_last_error;
pub extern "C" fn get_last_error() -> i32 {
    LAST_ERROR.load(Ordering::Acquire)
}

fn valid_buffer<T>(pointer: *const T, len: usize) -> bool {
    user_memory::valid(pointer, len)
}
unsafe fn bytes<'a>(pointer: *const u8, len: usize) -> Result<&'a [u8], SysError> {
    if !user_memory::valid_read(pointer, len) {
        return Err(SysError::InvalidArgument);
    }
    if len == 0 {
        return Ok(&[]);
    }
    // SAFETY: checked full range in live app storage; caller supplies initialized bytes.
    Ok(unsafe { slice::from_raw_parts(pointer, len) })
}
unsafe fn path<'a>(pointer: *const u8, len: usize) -> Result<&'a str, SysError> {
    if len == 0 || len > tufty_sys::MAX_PATH_COMPONENTS * tufty_sys::filename::NAME_CAPACITY {
        return Err(SysError::InvalidArgument);
    }
    // SAFETY: forwarded readable buffer contract; UTF-8 checked before making str.
    core::str::from_utf8(unsafe { bytes(pointer, len)? }).map_err(|_| SysError::InvalidArgument)
}

const _: Alloc = alloc;
pub unsafe extern "C" fn alloc(layout: CLayout) -> *mut u8 {
    report(
        Layout::try_from(layout)
            .map_err(|_| SysError::InvalidArgument)
            .and_then(user_memory::allocate),
        ptr::null_mut(),
    )
}
const _: Dealloc = dealloc;
pub unsafe extern "C" fn dealloc(pointer: *mut u8, layout: CLayout) {
    report(
        Layout::try_from(layout)
            .map_err(|_| SysError::InvalidArgument)
            .and_then(|layout| user_memory::deallocate(pointer, layout)),
        (),
    )
}
const _: Print = print;
pub unsafe extern "C" fn print(pointer: *const u8, len: usize) {
    // SAFETY: forwarded live-buffer contract.
    let result = unsafe { bytes(pointer, len) }.and_then(|buffer| {
        let _message = core::str::from_utf8(buffer).map_err(|_| SysError::InvalidArgument)?;
        #[cfg(feature = "defmt")]
        defmt::info!("print: {}", _message);
        Ok(())
    });
    report(result, ())
}
const _: SleepMs = sleep;
pub extern "C" fn sleep(ms: u64) {
    if let Some(duration) = Duration::try_from_millis(ms)
        && let Some(deadline) = Instant::now().checked_add(duration)
    {
        wait(Timer::at(deadline));
    } else {
        report(Err(SysError::InvalidArgument), ());
    }
}
pub static mut MS_SINCE_LAUNCH: Option<Instant> = None;
const _: GetMs = get_ms;
pub extern "C" fn get_ms() -> u64 {
    // SAFETY: only core1 writes before app entry and reads via this syscall.
    unsafe { MS_SINCE_LAUNCH }.map_or(0, |start| start.elapsed().as_millis())
}
const _: DrawIter = draw_iter;
pub unsafe extern "C" fn draw_iter(cpixels: *const CPixel, len: usize) {
    if !user_memory::valid_read(cpixels, len) {
        report(Err(SysError::InvalidArgument), ());
        return;
    }
    if len == 0 {
        return;
    }
    // SAFETY: checked shape and caller's readable CPixel buffer contract.
    let pixels = unsafe { slice::from_raw_parts(cpixels, len) };
    if let Some(framebuffer) = wait(FRAMEBUFFER.lock()).as_mut() {
        let _ = framebuffer.draw_iter(pixels.iter().copied().map(Into::into));
    }
}
const _: FillRect = fill_rect;
pub extern "C" fn fill_rect(x: u16, y: u16, width: u16, height: u16, color: u16) {
    let area = Rectangle::new(
        Point::new(x as i32, y as i32),
        Size::new(width as u32, height as u32),
    );
    if let Some(framebuffer) = wait(FRAMEBUFFER.lock()).as_mut() {
        let _ = framebuffer.fill_solid(&area, Rgb565::from(RawU16::new(color)));
    }
}
const _: Blit = blit;
pub unsafe extern "C" fn blit(
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    colors: *const u16,
    len: usize,
) {
    if !user_memory::valid_read(colors, len) || len > width as usize * height as usize {
        report(Err(SysError::InvalidArgument), ());
        return;
    }
    if len == 0 {
        return;
    }
    let area = Rectangle::new(
        Point::new(x as i32, y as i32),
        Size::new(width as u32, height as u32),
    );
    // SAFETY: checked shape and caller's readable storage. Short iterators are valid.
    let raw = unsafe { slice::from_raw_parts(colors, len) };
    if let Some(framebuffer) = wait(FRAMEBUFFER.lock()).as_mut() {
        let _ =
            framebuffer.fill_contiguous(&area, raw.iter().map(|&color| RawU16::new(color).into()));
    }
}
pub static KEY_CACHE: Channel<CriticalSectionRawMutex, KeyEvent, 32> = Channel::new();
const _: GetKey = get_key;
pub unsafe extern "C" fn get_key(output: *mut KeyEventC) {
    if !valid_buffer(output, 1) {
        report(Err(SysError::InvalidArgument), ());
        return;
    }
    let event = KEY_CACHE
        .try_receive()
        .unwrap_or(KeyEvent {
            key: KeyCode::Unknown(0),
            state: KeyState::Idle,
            mods: Modifiers::empty(),
        })
        .into();
    // SAFETY: output is a checked app-owned range; no references to it are retained.
    unsafe { output.write(event) };
    report(Ok(()), ())
}
const _: GenRand = gen_rand;
pub unsafe extern "C" fn gen_rand(request: *mut RngRequest) {
    if !valid_buffer(request, 1) {
        report(Err(SysError::InvalidArgument), ());
        return;
    }
    // SAFETY: repr(C) starts with a C enum tag; range and alignment checked first.
    let tag = unsafe { request.cast::<u32>().read() };
    if tag > 2 {
        report(Err(SysError::InvalidArgument), ());
        return;
    }
    let mut rng = RoscRng;
    // SAFETY: valid tag and mapped storage; raw pointers in Bytes admit any bit pattern.
    match unsafe { request.read() } {
        RngRequest::U32(_) => {
            // SAFETY: validated request storage; no borrowed references into it survive.
            unsafe { request.write(RngRequest::U32(rng.next_u32())) };
        }
        RngRequest::U64(_) => {
            // SAFETY: validated request storage; no borrowed references into it survive.
            unsafe { request.write(RngRequest::U64(rng.next_u64())) };
        }
        RngRequest::Bytes { ptr, len } => {
            if !valid_buffer(ptr, len)
                || (Region {
                    start: request as usize,
                    len: size_of::<RngRequest>(),
                })
                .overlaps(Region {
                    start: ptr as usize,
                    len,
                })
            {
                report(Err(SysError::InvalidArgument), ());
                return;
            }
            if len > 0 {
                // SAFETY: validated app storage disjoint from request; single app thread.
                rng.fill_bytes(unsafe { slice::from_raw_parts_mut(ptr, len) });
            }
        }
    }
    report(Ok(()), ())
}
const _: ListDir = list_dir;
pub unsafe extern "C" fn list_dir(
    directory: *const u8,
    len: usize,
    entries: *mut *mut c_char,
    count: usize,
    capacity: usize,
) -> usize {
    let result = (|| {
        // SAFETY: forwarded readable path contract; parser checks UTF-8.
        let directory = unsafe { path(directory, len)? };
        split_path(directory).ok_or(SysError::InvalidArgument)?;
        if capacity == 0 || count > 256 || !valid_buffer(entries, count) {
            return Err(SysError::InvalidArgument);
        }
        let entries = if count == 0 {
            &[]
        } else {
            // SAFETY: full pointer array is in live app storage; no mutable borrow is needed.
            unsafe { slice::from_raw_parts(entries, count) }
        };
        let pointer_array = Region {
            start: entries.as_ptr() as usize,
            len: size_of_val(entries),
        };
        let path_region = Region {
            start: directory.as_ptr() as usize,
            len: directory.len(),
        };
        for (index, &entry) in entries.iter().enumerate() {
            let output = Region {
                start: entry as usize,
                len: capacity,
            };
            if !valid_buffer(entry, capacity)
                || output.overlaps(pointer_array)
                || output.overlaps(path_region)
                || entries[..index].iter().any(|&previous| {
                    output.overlaps(Region {
                        start: previous as usize,
                        len: capacity,
                    })
                })
            {
                return Err(SysError::InvalidArgument);
            }
        }
        let names = crate::storage::list(directory)?;
        if names.len() > entries.len() || names.iter().any(|name| name.len() >= capacity) {
            return Err(SysError::BufferTooSmall);
        }
        for (name, &output) in names.iter().zip(entries) {
            // SAFETY: output buffers were validated and are disjoint from inputs and each other.
            unsafe {
                ptr::copy_nonoverlapping(name.as_ptr(), output.cast(), name.len());
                *output.add(name.len()) = 0;
            }
        }
        Ok(names.len())
    })();
    report(result, 0)
}
const _: ReadFile = read_file;
pub unsafe extern "C" fn read_file(
    file: *const u8,
    len: usize,
    offset: usize,
    buffer: *mut u8,
    buffer_len: usize,
) -> usize {
    let result = (|| {
        // SAFETY: forwarded readable path contract.
        let file = unsafe { path(file, len)? };
        if !valid_buffer(buffer, buffer_len)
            || (Region {
                start: file.as_ptr() as usize,
                len: file.len(),
            })
            .overlaps(Region {
                start: buffer as usize,
                len: buffer_len,
            })
        {
            return Err(SysError::InvalidArgument);
        }
        let buffer = if buffer_len == 0 {
            &mut []
        } else {
            // SAFETY: caller supplies exclusive writable storage; checked shape.
            unsafe { slice::from_raw_parts_mut(buffer, buffer_len) }
        };
        let data = crate::storage::file(file)?;
        let remaining = data.get(offset..).ok_or(SysError::InvalidArgument)?;
        let count = remaining.len().min(buffer.len());
        buffer[..count].copy_from_slice(&remaining[..count]);
        Ok(count)
    })();
    report(result, 0)
}
const _: WriteFile = write_file;
pub unsafe extern "C" fn write_file(
    file: *const u8,
    len: usize,
    offset: usize,
    buffer: *const u8,
    buffer_len: usize,
) {
    let result = (|| {
        // SAFETY: caller supplies both readable path and payload storage.
        let file = unsafe { path(file, len)? };
        let buffer = unsafe { bytes(buffer, buffer_len)? };
        let _ = (buffer, offset);
        split_path(file).ok_or(SysError::InvalidArgument)?;
        Err(SysError::Unsupported)
    })();
    report(result, ())
}
const _: FileLen = file_len;
pub unsafe extern "C" fn file_len(file: *const u8, len: usize) -> usize {
    // SAFETY: caller supplies readable path storage; errors are returned.
    let result = unsafe { path(file, len) }
        .and_then(|file| crate::storage::file(file).map(|bytes| bytes.len()));
    report(result, 0)
}
const _: ReconfigureAudioSampleRate = reconfigure_audio_sample_rate;
pub extern "C" fn reconfigure_audio_sample_rate(sample_rate: u32) {
    report(audio::configure(sample_rate), ())
}
const _: AudioBufferReady = audio_buffer_ready;
pub extern "C" fn audio_buffer_ready() -> bool {
    audio::ready()
}
const _: SendAudioBuffer = send_audio_buffer;
pub unsafe extern "C" fn send_audio_buffer(pointer: *const u8, len: usize) {
    // SAFETY: caller's readable buffer contract; submit copies before return.
    report(unsafe { bytes(pointer, len) }.and_then(audio::submit), ())
}

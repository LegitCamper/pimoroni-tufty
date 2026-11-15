#[cfg(target_os = "none")]
use core::alloc::{GlobalAlloc, Layout};
use embedded_alloc::LlffHeap;

#[repr(C, align(32))]
struct Arena<const SIZE: usize>([u8; SIZE]);

static mut KERNEL_ARENA: Arena<{ 48 * 1024 }> = Arena([0; 48 * 1024]);
static mut APP_ARENA: Arena<{ tufty_sys::memory::APP_ARENA_SIZE }> =
    Arena([0; tufty_sys::memory::APP_ARENA_SIZE]);
static KERNEL_HEAP: LlffHeap = LlffHeap::empty();
pub static APP_HEAP: LlffHeap = LlffHeap::empty();

pub fn init() {
    // SAFETY: called once before allocation/core1 startup; arenas are permanent and disjoint.
    unsafe {
        KERNEL_HEAP.init(core::ptr::addr_of_mut!(KERNEL_ARENA) as usize, 48 * 1024);
        APP_HEAP.init(
            core::ptr::addr_of_mut!(APP_ARENA) as usize,
            tufty_sys::memory::APP_ARENA_SIZE,
        );
    }
}

pub fn is_app_sram(pointer: *const u8) -> bool {
    let start = core::ptr::addr_of!(APP_ARENA) as usize;
    (start..start + tufty_sys::memory::APP_ARENA_SIZE).contains(&(pointer as usize))
}

#[cfg(target_os = "none")]
pub struct Allocator;

// SAFETY: core0 uses the permanent kernel heap, core1 uses tracked app SRAM.
// Deallocation routes by allocation address, not whichever core drops the value.
#[cfg(target_os = "none")]
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if embassy_rp::pac::SIO.cpuid().read() == 1 {
            crate::user_memory::allocate_sram(layout).unwrap_or(core::ptr::null_mut())
        } else {
            // SAFETY: initialized kernel heap, valid GlobalAlloc layout.
            unsafe { KERNEL_HEAP.alloc(layout) }
        }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if is_app_sram(pointer) {
            let _ = crate::user_memory::deallocate(pointer, layout);
        } else {
            // SAFETY: global allocations outside APP_ARENA belong to KERNEL_HEAP.
            unsafe { KERNEL_HEAP.dealloc(pointer, layout) };
        }
    }
}

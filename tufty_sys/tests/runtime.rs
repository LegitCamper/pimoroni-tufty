#[path = "../../kernel/src/runtime.rs"]
mod runtime;

use core::alloc::{GlobalAlloc, Layout};
use tufty_sys::memory::{APP_ARENA_SIZE, Allocation, AppMemory, Region};

#[test]
fn runtime_sram_is_reused_by_builtin_state_and_elf_images() {
    runtime::init();
    let initial_free = runtime::APP_HEAP.free();
    let state_layout = Layout::from_size_align(64 * 1024, 32).unwrap();
    let image_layout = Layout::from_size_align(120 * 1024, 256).unwrap();
    for elf in [false, true, false, true] {
        let mut memory = AppMemory::<4>::new();
        // SAFETY: heap initialized once; all allocations freed within this iteration.
        let image = if elf {
            unsafe { runtime::APP_HEAP.alloc_zeroed(image_layout) }
        } else {
            core::ptr::null_mut()
        };
        if elf {
            assert!(!image.is_null());
            assert_eq!(image as usize % 256, 0);
            memory.image = Region {
                start: image as usize,
                len: image_layout.size(),
            };
        }
        // SAFETY: valid nonempty layout; pointer checked before access.
        let state = unsafe { runtime::APP_HEAP.alloc_zeroed(state_layout) };
        assert!(!state.is_null());
        assert!(runtime::is_app_sram(state));
        let allocation = Allocation {
            address: state as usize,
            layout: state_layout,
        };
        assert!(memory.track(allocation));
        assert!(!memory.track(allocation));
        assert!(memory.permits(state as usize, state_layout.size(), 1, 1));
        assert!(!memory.permits(state as usize, state_layout.size() + 1, 1, 1));
        // SAFETY: the complete zeroed state is owned and bounds checked.
        unsafe {
            assert!(
                core::slice::from_raw_parts(state, state_layout.size())
                    .iter()
                    .all(|&byte| byte == 0)
            );
            state.write_bytes(0xa5, state_layout.size());
            assert!(
                runtime::APP_HEAP
                    .alloc(Layout::from_size_align(APP_ARENA_SIZE, 8).unwrap())
                    .is_null()
            );
        }
        while let Some(allocation) = memory.pop() {
            // SAFETY: exact live allocator record, released once on simulated app exit.
            unsafe {
                runtime::APP_HEAP.dealloc(allocation.address as *mut u8, allocation.layout);
            }
        }
        if elf {
            // SAFETY: exact live image allocation, after simulated app exit.
            unsafe {
                runtime::APP_HEAP.dealloc(image, image_layout);
            }
        }
        assert_eq!(runtime::APP_HEAP.used(), 0);
        assert_eq!(runtime::APP_HEAP.free(), initial_free);
    }
}

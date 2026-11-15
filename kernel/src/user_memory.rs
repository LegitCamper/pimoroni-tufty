use core::{alloc::Layout, cell::RefCell};
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use tufty_sys::{
    SysError,
    memory::{Allocation, AppMemory, Region},
};
use {crate::heap::HEAP, core::alloc::GlobalAlloc};

// ponytail: 256 live app allocations; raise the bound if measured app demand needs it.
static MEMORY: Mutex<CriticalSectionRawMutex, RefCell<AppMemory<256>>> =
    Mutex::new(RefCell::new(AppMemory::new()));

pub fn begin(image: Region, stack: Region) {
    MEMORY.lock(|memory| {
        let mut memory = memory.borrow_mut();
        memory.image = image;
        memory.stack = stack;
    });
}

pub fn begin_builtin(stack: Region) {
    begin(Region::default(), stack);
    MEMORY.lock(|memory| {
        let mut memory = memory.borrow_mut();
        memory.read_only = Region {
            start: 0x1000_0000,
            len: 4 * 1024 * 1024,
        };
        memory.shared_read = tufty::display::builtin_buffer_region();
    });
}

pub fn valid_read<T>(pointer: *const T, len: usize) -> bool {
    MEMORY.lock(|memory| {
        memory
            .borrow()
            .permits_read(pointer as usize, len, size_of::<T>(), align_of::<T>())
    })
}

pub fn valid<T>(pointer: *const T, len: usize) -> bool {
    MEMORY.lock(|memory| {
        memory
            .borrow()
            .permits(pointer as usize, len, size_of::<T>(), align_of::<T>())
    })
}

pub fn allocate(layout: Layout) -> Result<*mut u8, SysError> {
    allocate_from(layout, &HEAP)
}

pub fn allocate_sram(layout: Layout) -> Result<*mut u8, SysError> {
    allocate_from(layout, &crate::runtime::APP_HEAP)
}

fn allocate_from(layout: Layout, heap: &embedded_alloc::LlffHeap) -> Result<*mut u8, SysError> {
    if layout.size() == 0 {
        return Err(SysError::InvalidArgument);
    }
    // SAFETY: layout is nonempty; both allocators are initialized before app entry.
    let pointer = unsafe { heap.alloc(layout) };
    if pointer.is_null() {
        return Err(SysError::NoMemory);
    }
    let allocation = Allocation {
        address: pointer as usize,
        layout,
    };
    if !MEMORY.lock(|memory| memory.borrow_mut().track(allocation)) {
        free(allocation);
        return Err(SysError::NoMemory);
    }
    Ok(pointer)
}

pub fn deallocate(pointer: *mut u8, layout: Layout) -> Result<(), SysError> {
    let allocation = Allocation {
        address: pointer as usize,
        layout,
    };
    if !MEMORY.lock(|memory| memory.borrow_mut().release(allocation)) {
        return Err(SysError::InvalidArgument);
    }
    free(allocation);
    Ok(())
}

fn free(allocation: Allocation) {
    // SAFETY: exact live allocation/layout, either unpublished or removed from tracking.
    unsafe {
        let pointer = allocation.address as *mut u8;
        if crate::runtime::is_app_sram(pointer) {
            crate::runtime::APP_HEAP.dealloc(pointer, allocation.layout);
        } else {
            HEAP.dealloc(pointer, allocation.layout);
        }
    }
}

pub fn finish() {
    while let Some(allocation) = MEMORY.lock(|memory| memory.borrow_mut().pop()) {
        free(allocation);
    }
    begin(Region::default(), Region::default());
    MEMORY.lock(|memory| {
        let mut memory = memory.borrow_mut();
        memory.read_only = Region::default();
        memory.shared_read = Region::default();
    });
}

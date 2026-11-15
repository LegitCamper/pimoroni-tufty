use embedded_alloc::LlffHeap;

pub static HEAP: LlffHeap = LlffHeap::empty();

pub fn init_qmi_psram_heap(size: u32) {
    // SAFETY: called once after QMI validation, before core1 or any PSRAM allocation.
    unsafe {
        HEAP.init(0x1100_0000, size as usize);
    }
}

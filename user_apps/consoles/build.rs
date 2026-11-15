fn main() {
    let mut build = cc::Build::new();
    build
        .compiler("arm-none-eabi-gcc")
        .warnings(false)
        .no_default_flags(true)
        .flag("-O3")
        .flag("-g")
        .flag("-mcpu=cortex-m33")
        .flag("-mthumb")
        .flag("-mfloat-abi=hard")
        .flag("-mfpu=fpv5-sp-d16")
        .flag("-std=gnu11")
        .flag("-fno-strict-aliasing")
        .flag("-fwrapv")
        .flag("-ffunction-sections")
        .flag("-fdata-sections")
        .flag("-fstack-usage")
        .flag("-Wframe-larger-than=8192")
        .flag("-Werror=frame-larger-than=8192")
        .define("NDEBUG", None)
        .include("native");
    for name in ["nes", "snes", "gbc", "gba", "portable"] {
        build.file(format!("native/{name}.c"));
    }
    for name in [
        "snes", "cpu", "apu", "spc", "dsp", "dma", "ppu", "cart", "input",
    ] {
        build.file(format!("vendor/lakesnes/{name}.c"));
    }
    build.compile("tufty_consoles");
    println!("cargo:rerun-if-changed=native");
    println!("cargo:rerun-if-changed=vendor");
}

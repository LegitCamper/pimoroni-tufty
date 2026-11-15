use std::{path::Path, process::Command};

fn main() {
    let flags = [
        "-mcpu=cortex-m33",
        "-mthumb",
        "-mfloat-abi=hard",
        "-mfpu=fpv5-sp-d16",
    ];
    let mut build = cc::Build::new();
    build
        .compiler("arm-none-eabi-gcc")
        .no_default_flags(true)
        .flags(flags)
        .flag("-O2")
        .flag("-std=c11")
        .flag("-ffunction-sections")
        .flag("-fdata-sections")
        .flag("-fstack-usage")
        .flag("-Wno-unused-function")
        .flag("-Wframe-larger-than=4096")
        .flag("-Werror=frame-larger-than=4096")
        .flag("-include")
        .flag("native/config.h")
        .include("vendor/lua")
        .file("native/runner.c");
    for file in std::fs::read_to_string("native/sources.txt")
        .expect("Lua source manifest")
        .split_whitespace()
    {
        build.file(format!("vendor/lua/{file}.c"));
    }
    build.compile("tufty_love");
    for library in ["m", "c", "nosys"] {
        let output = Command::new("arm-none-eabi-gcc")
            .args(flags)
            .arg(format!("--print-file-name=lib{library}.a"))
            .output()
            .expect("ARM libraries");
        assert!(output.status.success(), "ARM library lookup failed");
        let path = String::from_utf8(output.stdout).expect("ARM library path");
        let path = Path::new(path.trim());
        assert!(path.is_file(), "missing ARM library: {}", path.display());
        println!(
            "cargo:rustc-link-search=native={}",
            path.parent().unwrap().display()
        );
        println!("cargo:rustc-link-lib=static={library}");
    }
    println!("cargo:rerun-if-changed=native");
    println!("cargo:rerun-if-changed=vendor");
}

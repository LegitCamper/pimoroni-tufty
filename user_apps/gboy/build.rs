//! This build script copies the `memory.x` file from the crate root into
//! a directory where the linker can always find it at build time.
//! For many projects this is optional, as the linker always searches the
//! project root directory -- wherever `Cargo.toml` is. However, if you
//! are using a workspace or have a more complicated build setup, this
//! build script becomes required. Additionally, by requesting that
//! Cargo re-run the build script whenever `memory.x` is changed,
//! updating `memory.x` ensures a rebuild of the application with the
//! new memory settings.

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    bindgen();
    if env::var_os("CARGO_FEATURE_BUILTIN").is_some() {
        return;
    }

    // Put `memory.x` in our output directory and ensure it's
    // on the linker search path.
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("../memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    println!("cargo:rerun-if-changed=../memory.x");
    println!("cargo:rustc-link-arg-bins=-Tmemory.x");
}

fn bindgen() {
    let include = PathBuf::from(
        env::var_os("ARM_INCLUDE").unwrap_or_else(|| "/usr/arm-none-eabi/include".into()),
    );
    let defines = [
        ("PEANUT_GB_IS_LITTLE_ENDIAN", "1"),
        ("ENABLE_LCD", "1"),
        ("ENABLE_SOUND", "0"),
        ("PEANUT_GB_12_COLOUR", "0"),
        ("PEANUT_GB_HIGH_LCD_ACCURACY", "0"),
    ];
    println!("cargo:rerun-if-env-changed=ARM_INCLUDE");
    println!("cargo:rerun-if-changed=peanut_gb_stub.c");
    println!("cargo:rerun-if-changed=Peanut-GB/peanut_gb.h");
    let mut bindings = bindgen::Builder::default()
        .header("peanut_gb_stub.c")
        .clang_arg("--target=arm-none-eabi")
        .clang_arg("-mcpu=cortex-m33")
        .clang_arg("-mfloat-abi=hard")
        .clang_arg("-mfpu=fpv5-sp-d16")
        .clang_arg("-IPeanut-GB")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .disable_nested_struct_naming()
        .clang_arg(format!("-I{}", include.display()))
        .allowlist_function("gb_.*")
        .allowlist_var("JOYPAD_.*")
        .use_core();
    let mut compiler = cc::Build::new();
    for (name, value) in defines {
        bindings = bindings.clang_arg(format!("-D{name}={value}"));
        compiler.define(name, value);
    }

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    compiler
        .file("peanut_gb_stub.c")
        .include("Peanut-GB")
        .include(&include)
        .pic(true)
        .opt_level(3)
        .flag("-fdata-sections")
        .flag("-ffunction-sections")
        .flag("-g0")
        .compile("peanut_gb");
}

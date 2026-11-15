#[allow(dead_code)]
#[path = "../../kernel/src/elf_format.rs"]
mod format;
#[allow(dead_code)]
#[path = "../../user_apps/snake/src/timing.rs"]
mod snake_timing;
#[allow(dead_code)]
#[path = "../../user_apps/gboy/src/video.rs"]
mod video;

#[test]
#[ignore = "requires just kernel; run just test-images"]
fn builtins_execute_from_flash_without_per_emulator_sram_reservations() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let bytes =
        std::fs::read(root.join("target/thumbv8m.main-none-eabihf/release/tufty-kernel")).unwrap();
    assert_eq!(&bytes[..7], b"\x7fELF\x01\x01\x01");
    let half = |offset| u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
    let word = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    assert_eq!(half(16), 2);
    assert_eq!(half(18), 40);
    assert_eq!(half(46), 40);
    let mut sections = Vec::new();
    for index in 0..half(48) {
        let start = word(32) as usize + index as usize * 40;
        sections.push(
            format::SectionHeader::parse(
                bytes[start..start + 40].try_into().unwrap(),
                bytes.len() as u32,
                half(48),
            )
            .unwrap(),
        );
    }
    let mut symbols = Vec::new();
    for section in &sections {
        if section.kind != format::SHT_SYMTAB {
            continue;
        }
        let strings = &sections[section.link as usize];
        let strings = &bytes[strings.offset as usize..(strings.offset + strings.size) as usize];
        for record in bytes[section.offset as usize..(section.offset + section.size) as usize]
            .chunks_exact(16)
        {
            let symbol = format::Symbol::parse(record.try_into().unwrap());
            let name = strings[symbol.name as usize..]
                .split(|&byte| byte == 0)
                .next()
                .unwrap();
            symbols.push((std::str::from_utf8(name).unwrap(), symbol));
        }
    }
    for entry in [
        "4gboy3run",
        "5chip83run",
        "5snake4main",
        "8consoles3nes",
        "8consoles4snes",
        "8consoles3gbc",
        "8consoles3gba",
    ] {
        let (_, symbol) = symbols
            .iter()
            .find(|(name, _)| name.contains(entry))
            .expect("builtin entry must remain linked");
        assert!((0x10000000..0x10400000).contains(&symbol.address));
        assert_eq!(symbol.address & 1, 1);
    }
    for (name, size) in [
        ("9APP_ARENA", tufty_sys::memory::APP_ARENA_SIZE),
        ("12KERNEL_ARENA", 48 * 1024),
        ("SYS_CALL_TABLE", tufty_sys::SYS_CALL_TABLE_COUNT * 4),
    ] {
        let (_, symbol) = symbols
            .iter()
            .find(|(symbol, _)| symbol.contains(name))
            .unwrap();
        assert_eq!(symbol.size as usize, size);
        assert!((0x20000000..0x20080000).contains(&symbol.address));
    }
    for (name, _) in symbols {
        assert!(
            !name.contains("GAME_BOY")
                && !name.contains("GAME_ROM")
                && !name.contains("minigb_apu")
                && !name.contains("arm7_lookup_table")
                && !name.contains("arm7t_lookup_table")
        );
    }
}

#[test]
#[ignore = "requires just apps; run just test-images"]
fn built_apps_match_loader_and_icon_contracts() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    for name in ["snake", "gboy", "chip8"] {
        let bytes = std::fs::read(
            root.join("target/thumbv8m.main-none-eabihf/release-binary")
                .join(name),
        )
        .unwrap();
        let header =
            format::Header::parse(bytes[..52].try_into().unwrap(), bytes.len() as u32).unwrap();
        let mut segments = Vec::new();
        for index in 0..header.program_count {
            let start = header.program_offset(index).unwrap() as usize;
            let program = format::ProgramHeader::parse(
                bytes[start..start + 32].try_into().unwrap(),
                bytes.len() as u32,
            )
            .unwrap();
            if program.kind == format::PT_LOAD && program.memory_size != 0 {
                segments.push(program);
            }
        }
        let image = format::Image::new(&segments, header.entry).unwrap();
        let metadata = tufty_sys::app::from_elf(&bytes).expect("missing .tufty_app");
        let icon = tufty_sys::png::decode(metadata.icon.expect("missing linked PNG")).unwrap();
        assert_eq!((icon.width, icon.height), (32, 32));
        let mut sections = Vec::new();
        for index in 0..header.section_count {
            let start = header.section_offset(index).unwrap() as usize;
            sections.push(
                format::SectionHeader::parse(
                    bytes[start..start + 40].try_into().unwrap(),
                    bytes.len() as u32,
                    header.section_count,
                )
                .unwrap(),
            );
        }
        let mut found = false;
        for section in &sections {
            if section.flags & format::SHF_ALLOC != 0 && section.size != 0 {
                image.range(section.address, section.size, 0).unwrap();
            }
            assert_ne!(section.kind, format::SHT_RELA);
            if section.kind == format::SHT_REL {
                let target = (section.info != 0).then(|| &sections[section.info as usize]);
                for relocation in bytes
                    [section.offset as usize..(section.offset + section.size) as usize]
                    .chunks_exact(8)
                {
                    image
                        .relocation_range(relocation.try_into().unwrap(), target)
                        .unwrap();
                }
            }
        }
        for symbols in &sections {
            if !matches!(symbols.kind, format::SHT_SYMTAB | format::SHT_DYNSYM) {
                continue;
            }
            let names = &sections[symbols.link as usize];
            let names = &bytes[names.offset as usize..(names.offset + names.size) as usize];
            for record in bytes[symbols.offset as usize..(symbols.offset + symbols.size) as usize]
                .chunks_exact(16)
            {
                let symbol = format::Symbol::parse(record.try_into().unwrap());
                if names[symbol.name as usize..].starts_with(b"SYS_CALL_TABLE\0") {
                    image
                        .syscall_range(
                            &symbol,
                            &sections[symbol.section as usize],
                            tufty_sys::SYS_CALL_TABLE_COUNT,
                        )
                        .unwrap();
                    found = true;
                }
            }
        }
        assert!(found, "missing syscall table in {name}");
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Console {
    Nes,
    Snes,
    GameBoyColor,
    GameBoyAdvance,
}

pub const MAX_ROM: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cartridge {
    pub offset: usize,
    pub mapper: u32,
    pub ram_size: u32,
    pub pal: bool,
}

pub fn validate(console: Console, rom: &[u8]) -> Result<Cartridge, &'static str> {
    if rom.len() > MAX_ROM + 512 {
        return Err("ROM exceeds 4 MiB limit");
    }
    let mut cartridge = Cartridge {
        offset: 0,
        mapper: 0,
        ram_size: 0,
        pal: false,
    };
    match console {
        Console::Nes => {
            if rom.len() < 16 || &rom[..4] != b"NES\x1a" {
                return Err("Expected an iNES cartridge");
            }
            if rom[7] & 15 != 0 || rom[6] & 12 != 0 || rom[9] & 1 != 0 || rom[10] & 3 != 0 {
                return Err("NES 2.0, trainer, PAL or four-screen unsupported");
            }
            let prg = rom[4] as usize;
            let chr = rom[5] as usize;
            cartridge.mapper = u32::from((rom[6] >> 4) | (rom[7] & 0xf0));
            if !prg.is_power_of_two() || (chr != 0 && !chr.is_power_of_two()) || rom[8] > 1 {
                return Err("Unsupported NES bank layout");
            }
            let supported = match cartridge.mapper {
                0 => prg <= 2 && chr <= 1,
                1 => prg <= 16 && chr <= 16,
                2 => prg <= 16 && chr == 0,
                4 => (2..=32).contains(&prg) && chr <= 32,
                _ => false,
            };
            if !supported {
                return Err("Supported NES mappers: 0, 1, 2, 4");
            }
            if rom.len() != 16 + prg * 16384 + chr * 8192 {
                return Err("Truncated or oversized iNES data");
            }
        }
        Console::Snes => {
            cartridge.offset = if rom.len() % 32768 == 512 { 512 } else { 0 };
            let data = &rom[cartridge.offset..];
            if data.len() < 32768 || data.len() > MAX_ROM || !data.len().is_power_of_two() {
                return Err("SNES requires power-of-two ROM, 32 KiB-4 MiB");
            }
            let mut candidate = None;
            for (offset, mapper) in [(0x7fc0, 1), (0xffc0, 2)] {
                let Some(header) = data.get(offset..offset + 64) else {
                    continue;
                };
                if !matches!((mapper, header[21]), (1, 0x20 | 0x30) | (2, 0x21 | 0x31)) {
                    continue;
                }
                if header[22] > 2 || header[24] > 7 || !(5..=12).contains(&header[23]) {
                    continue;
                }
                if (1024usize << header[23]) != data.len() {
                    continue;
                }
                let checksum = u16::from_le_bytes([header[30], header[31]]);
                let complement = u16::from_le_bytes([header[28], header[29]]);
                let reset = u16::from_le_bytes([header[60], header[61]]);
                if checksum ^ complement != 0xffff || reset < 0x8000 {
                    continue;
                }
                if candidate.is_some() {
                    return Err("Ambiguous SNES mapping headers");
                }
                candidate = Some((mapper, header));
            }
            let Some((mapper, header)) = candidate else {
                return Err("Need plain LoROM/HiROM; no enhancement chips");
            };
            cartridge.mapper = mapper;
            cartridge.ram_size = if header[22] != 0 && header[24] != 0 {
                1024 << header[24]
            } else {
                0
            };
            cartridge.pal = (2..=12).contains(&header[25]);
        }
        Console::GameBoyColor => {
            if rom.len() < 32768 || rom[0x148] > 7 || rom.len() != 32768usize << rom[0x148] {
                return Err("GB/GBC ROM size does not match header");
            }
            if rom[0x143] >= 0x80 && !matches!(rom[0x143], 0x80 | 0xc0) {
                return Err("Invalid GB color-mode flag");
            }
            if !matches!(rom[0x147], 0..=3 | 5..=6 | 0x0f..=0x13 | 0x19..=0x1e) || rom[0x149] > 5 {
                return Err("Supported GB mappers: ROM, MBC1/2/3/5");
            }
            let checksum = rom[0x134..=0x14c]
                .iter()
                .fold(0u8, |sum, byte| sum.wrapping_sub(*byte).wrapping_sub(1));
            if checksum != rom[0x14d] {
                return Err("Invalid GB header checksum");
            }
        }
        Console::GameBoyAdvance => {
            if rom.len() < 192 || rom.len() > MAX_ROM || rom.len() % 4 != 0 || rom[0xb2] != 0x96 {
                return Err("GBA ROM must be aligned, 192 bytes-4 MiB");
            }
            let checksum = rom[0xa0..=0xbc]
                .iter()
                .fold(0u8, |sum, byte| sum.wrapping_sub(*byte))
                .wrapping_sub(0x19);
            if checksum != rom[0xbd] {
                return Err("Invalid GBA header checksum");
            }
        }
    }
    Ok(cartridge)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;

    #[test]
    fn all_short_headers_rejected_without_panics() {
        for console in [
            Console::Nes,
            Console::Snes,
            Console::GameBoyColor,
            Console::GameBoyAdvance,
        ] {
            for len in 0..192 {
                assert!(validate(console, &vec![0; len]).is_err());
            }
        }
    }

    #[test]
    fn nes_rejects_zero_banks_trainers_and_truncation() {
        let mut rom = vec![0; 16 + 16384];
        rom[..4].copy_from_slice(b"NES\x1a");
        assert!(validate(Console::Nes, &rom).is_err());
        rom[4] = 1;
        assert!(validate(Console::Nes, &rom).is_ok());
        rom[6] = 4;
        assert!(validate(Console::Nes, &rom).is_err());
        rom[6] = 0;
        assert!(validate(Console::Nes, &rom[..rom.len() - 1]).is_err());
    }

    #[test]
    fn snes_checks_mapping_ram_exponent_and_copier_header() {
        let mut rom = vec![0; 32768 + 512];
        let header = &mut rom[512 + 0x7fc0..];
        header[21] = 0x20;
        header[23] = 5;
        header[28..30].copy_from_slice(&[255, 255]);
        header[61] = 0x80;
        assert_eq!(validate(Console::Snes, &rom).unwrap().offset, 512);
        rom[512 + 0x7fc0 + 24] = 255;
        assert!(validate(Console::Snes, &rom).is_err());
    }

    #[test]
    fn gb_and_gba_check_declared_size_and_checksum() {
        let mut rom = vec![0; 32768];
        rom[0x14d] = 0u8.wrapping_sub(25);
        assert!(validate(Console::GameBoyColor, &rom).is_ok());
        rom[0x148] = 7;
        assert!(validate(Console::GameBoyColor, &rom).is_err());
        let mut rom = vec![0; 192];
        rom[0xb2] = 0x96;
        rom[0xbd] = 0u8.wrapping_sub(0x96).wrapping_sub(0x19);
        assert!(validate(Console::GameBoyAdvance, &rom).is_ok());
        rom[0xbd] ^= 1;
        assert!(validate(Console::GameBoyAdvance, &rom).is_err());
    }
}

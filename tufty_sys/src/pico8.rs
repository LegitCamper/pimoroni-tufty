//! Strict text-cartridge decoding; no PNG decompressor or filesystem dependencies.

use crate::love::{MAX_SOURCE, validate_source};

pub const MAX_CART: usize = 160 * 1024;
pub const ASSET_BYTES: usize = 0x3100;
const SECTIONS: [&str; 7] = [
    "__lua__",
    "__gfx__",
    "__map__",
    "__gff__",
    "__sfx__",
    "__music__",
    "__label__",
];

pub struct Cartridge<'a> {
    pub source: &'a str,
    sections: [Option<&'a str>; 7],
}

fn hex(byte: u8) -> Result<u8, &'static str> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err("Invalid cartridge hex digit"),
    }
}

pub fn parse(bytes: &[u8]) -> Result<Cartridge<'_>, &'static str> {
    if bytes.len() > MAX_CART {
        return Err("P8 cartridge exceeds 160 KiB");
    }
    let text =
        core::str::from_utf8(bytes).map_err(|_| "P8 cartridge must be UTF-8 text, not PNG")?;
    if bytes.contains(&0) {
        return Err("NUL in P8 cartridge");
    }
    let mut lines = text.split_inclusive('\n');
    let header = lines.next().ok_or("Missing P8 header")?;
    if header.trim_end_matches(['\r', '\n']) != "pico-8 cartridge // http://www.pico-8.com" {
        return Err("Expected text .p8 cartridge header");
    }
    let version = lines.next().ok_or("Missing P8 version")?;
    let version_text = version
        .trim_end_matches(['\r', '\n'])
        .strip_prefix("version ")
        .ok_or("Missing P8 version")?;
    if version_text.is_empty() || !version_text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("Invalid P8 version");
    }
    let number: u8 = version_text.parse().map_err(|_| "Invalid P8 version")?;
    if !(1..=42).contains(&number) {
        return Err("P8 versions 1..42 supported");
    }
    let mut sections = [None; 7];
    let mut offset = header.len() + version.len();
    let mut current = None;
    let mut start = offset;
    for raw_line in lines {
        let line = raw_line.trim_end_matches(['\r', '\n']);
        if line.starts_with("__") && line.ends_with("__") {
            let section = SECTIONS
                .iter()
                .position(|&name| name == line)
                .ok_or("Unsupported P8 section")?;
            if let Some(index) = current {
                sections[index] = Some(&text[start..offset]);
            }
            if sections[section].is_some() {
                return Err("Duplicate P8 section");
            }
            if current.is_none() && section != 0 {
                return Err("P8 Lua section must come first");
            }
            current = Some(section);
            start = offset + raw_line.len();
        } else if current.is_none() && !line.is_empty() {
            return Err("Unexpected text before P8 Lua section");
        }
        offset += raw_line.len();
    }
    if let Some(index) = current {
        sections[index] = Some(&text[start..offset]);
    }
    let source = sections[0].ok_or("Missing P8 Lua section")?;
    validate_source(source.as_bytes())?;
    for (index, section) in sections.iter().enumerate().skip(1) {
        let Some(section) = section else {
            continue;
        };
        let (columns, rows) = match index {
            1 | 6 => (128, 128),
            2 => (256, 32),
            3 => (256, 2),
            4 => (168, 64),
            5 => (11, 64),
            _ => unreachable!(),
        };
        let mut count = 0;
        for line in section.lines().filter(|line| !line.is_empty()) {
            count += 1;
            if count > rows || line.len() != columns {
                return Err("Invalid P8 asset row size or count");
            }
            for (column, byte) in line.bytes().enumerate() {
                if index == 5 && column == 2 {
                    if byte != b' ' {
                        return Err("Invalid P8 music row");
                    }
                } else {
                    hex(byte)?;
                }
            }
        }
    }
    Ok(Cartridge { source, sections })
}

impl Cartridge<'_> {
    pub fn decode_assets(&self, output: &mut [u8]) -> Result<(), &'static str> {
        if output.len() != ASSET_BYTES {
            return Err("Wrong P8 asset buffer size");
        }
        output.fill(0);
        for section in 1..=3 {
            let Some(text) = self.sections[section] else {
                continue;
            };
            let base = match section {
                1 => 0,
                2 => 0x2000,
                _ => 0x3000,
            };
            for (row, line) in text.lines().filter(|line| !line.is_empty()).enumerate() {
                for (column, pair) in line.as_bytes().chunks_exact(2).enumerate() {
                    let first = hex(pair[0])?;
                    let second = hex(pair[1])?;
                    let value = if section == 1 {
                        first | second << 4
                    } else {
                        first << 4 | second
                    };
                    let stride = if section == 1 { 64 } else { 128 };
                    output[base + row * stride + column] = value;
                }
            }
        }
        Ok(())
    }
}

/// Convert only P8's != token, preserving strings, comments, offsets and line numbers.
pub fn normalize_source(source: &str, output: &mut [u8]) -> Result<(), &'static str> {
    validate_source(source.as_bytes())?;
    if output.len() != source.len() || output.len() > MAX_SOURCE {
        return Err("Wrong Lua source buffer size");
    }
    output.copy_from_slice(source.as_bytes());
    let bytes = source.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        let comment = bytes[cursor..].starts_with(b"--");
        let opening = cursor + if comment { 2 } else { 0 };
        if let Some(bracket) = long_bracket(bytes, opening) {
            let mut end = opening + bracket;
            while end < bytes.len() {
                if bytes[end] == b']' && long_close(bytes, end, bracket) {
                    break;
                }
                end += 1;
            }
            cursor = (end + bracket).min(bytes.len());
        } else if comment {
            cursor += 2;
            while cursor < bytes.len() && bytes[cursor] != b'\n' {
                cursor += 1;
            }
        } else if matches!(bytes[cursor], b'\'' | b'"') {
            let quote = bytes[cursor];
            cursor += 1;
            while cursor < bytes.len() {
                if bytes[cursor] == b'\\' {
                    cursor = (cursor + 2).min(bytes.len());
                } else if bytes[cursor] == quote {
                    cursor += 1;
                    break;
                } else {
                    cursor += 1;
                }
            }
        } else {
            if bytes[cursor..].starts_with(b"!=") {
                output[cursor] = b'~';
            }
            cursor += 1;
        }
    }
    Ok(())
}

fn long_bracket(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'[') {
        return None;
    }
    let mut end = start + 1;
    while bytes.get(end) == Some(&b'=') {
        end += 1;
    }
    (bytes.get(end) == Some(&b'[')).then_some(end - start + 1)
}

fn long_close(bytes: &[u8], start: usize, width: usize) -> bool {
    bytes.get(start + width - 1) == Some(&b']')
        && bytes
            .get(start + 1..start + width - 1)
            .is_some_and(|middle| middle.iter().all(|&byte| byte == b'='))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{format, vec};

    const HEADER: &str = "pico-8 cartridge // http://www.pico-8.com\nversion 42\n__lua__\nfunction _draw() cls(1) end\n";

    #[test]
    fn sprite_nibbles_map_and_flags_decode_without_overlap() {
        let text = format!(
            "{HEADER}__gfx__\n12{}\n__map__\nab{}\n__gff__\n80{}\n",
            "0".repeat(126),
            "0".repeat(254),
            "0".repeat(254)
        );
        let cart = parse(text.as_bytes()).unwrap();
        let mut assets = vec![0xff; ASSET_BYTES];
        cart.decode_assets(&mut assets).unwrap();
        assert_eq!(assets[0], 0x21);
        assert_eq!(assets[0x2000], 0xab);
        assert_eq!(assets[0x3000], 0x80);
        assert_eq!(assets[0x1000], 0);
        assert!(cart.decode_assets(&mut assets[..1]).is_err());
    }

    #[test]
    fn malformed_assets_headers_and_duplicate_sections_fail() {
        for suffix in ["__lua__\nx=1", "__gfx__\n123", "__other__\n", "__map__\ngg"] {
            assert!(parse(format!("{HEADER}{suffix}").as_bytes()).is_err());
        }
        assert!(parse(HEADER.replace("version 42", "version 43").as_bytes()).is_err());
        assert!(parse(b"\x89PNG\r\n").is_err());
        assert!(parse(&vec![b'x'; MAX_CART + 1]).is_err());
        assert!(
            parse(
                format!(
                    "{HEADER}__gfx__\n{}",
                    format!("{}\n", "0".repeat(128)).repeat(129)
                )
                .as_bytes()
            )
            .is_err()
        );
        assert!(parse(HEADER.replace('\n', "\r\n").as_bytes()).is_ok());
    }

    #[test]
    fn inequality_rewrite_preserves_literals_and_comments() {
        let source = "if a!=b then print('!=',\"!=\",[=[!=]=]) end -- !=\n--[==[!=]==]\nx!=y";
        let mut output = vec![0; source.len()];
        normalize_source(source, &mut output).unwrap();
        assert_eq!(
            core::str::from_utf8(&output).unwrap(),
            "if a~=b then print('!=',\"!=\",[=[!=]=]) end -- !=\n--[==[!=]==]\nx~=y"
        );
        assert!(normalize_source(source, &mut output[..1]).is_err());
    }

    #[test]
    fn bundled_demo_parses_and_normalizes() {
        let cart = parse(include_bytes!("../../user_apps/love/demo.p8")).unwrap();
        let mut assets = vec![0; ASSET_BYTES];
        let mut source = vec![0; cart.source.len()];
        cart.decode_assets(&mut assets).unwrap();
        normalize_source(cart.source, &mut source).unwrap();
        assert!(
            core::str::from_utf8(&source)
                .unwrap()
                .contains("function _draw()")
        );
    }
}

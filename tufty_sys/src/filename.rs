pub const MAX_NAME_UNITS: usize = 255;
pub const MAX_NAME_BYTES: usize = MAX_NAME_UNITS * 3;
pub const NAME_CAPACITY: usize = MAX_NAME_BYTES + 1;

pub fn nul_terminated_name(bytes: &[u8]) -> Option<&str> {
    let length = bytes.iter().position(|&byte| byte == 0)?;
    let name = core::str::from_utf8(&bytes[..length]).ok()?;
    valid_name(name).then_some(name)
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.ends_with([' ', '.'])
        && name.encode_utf16().count() <= MAX_NAME_UNITS
        && !name.chars().any(|character| {
            character <= '\u{1f}'
                || matches!(
                    character,
                    '"' | '*' | '/' | ':' | '<' | '>' | '?' | '\\' | '|' | '\u{ffff}'
                )
        })
}

pub fn write_label(
    output: &mut impl core::fmt::Write,
    name: &str,
    columns: usize,
) -> core::fmt::Result {
    let length = name.chars().count();
    if length <= columns {
        return output.write_str(name);
    }
    if columns < 5 {
        for character in name.chars().take(columns) {
            output.write_char(character)?;
        }
        return Ok(());
    }
    let tail = (columns / 3).min(10);
    for character in name.chars().take(columns - tail - 3) {
        output.write_char(character)?;
    }
    output.write_str("...")?;
    for character in name.chars().skip(length - tail) {
        output.write_char(character)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cartridge_titles_require_terminator_and_safe_path_component() {
        assert_eq!(
            nul_terminated_name(b"1234567890123456\0"),
            Some("1234567890123456")
        );
        assert_eq!(nul_terminated_name(b"TITLE\0garbage"), Some("TITLE"));
        for invalid in [
            &b"unterminated"[..],
            &b"../save\0"[..],
            &b"\xff\0"[..],
            &b"\0"[..],
        ] {
            assert_eq!(nul_terminated_name(invalid), None);
        }
    }

    #[test]
    fn names_validate_utf16_limits_and_separators() {
        assert_eq!(NAME_CAPACITY, 766);
        assert!(valid_name("My holiday photos 2026.bmp"));
        assert!(valid_name(&"\u{754c}".repeat(255)));
        assert!(valid_name(&"\u{1f4f7}".repeat(127)));
        assert!(!valid_name(&"\u{1f4f7}".repeat(128)));
        for invalid in [
            "",
            ".",
            "..",
            "bad/part",
            "bad\\part",
            "bad\0name",
            "bad:name",
            "bad?name",
            "tail.",
            "tail ",
        ] {
            assert!(!valid_name(invalid), "{invalid:?}");
        }
    }

    #[test]
    fn labels_preserve_unicode_and_distinguishing_suffix() {
        let mut label = std::string::String::new();
        write_label(&mut label, "Holiday photograph number 123.bmp", 20).unwrap();
        assert_eq!(label, "Holiday pho...23.bmp");
        label.clear();
        write_label(&mut label, &"\u{754c}".repeat(40), 29).unwrap();
        assert_eq!(label.chars().count(), 29);
        label.clear();
        write_label(&mut label, "Game Boy.bin", 29).unwrap();
        assert_eq!(label, "Game Boy.bin");
    }
}

pub const MAGIC: [u8; 8] = *b"TUFTYAPP";
pub const HEADER_SIZE: usize = 48;
pub const MAX_ICON_BYTES: usize = 32768;

#[repr(u32)]
pub enum IconKind {
    None = 0,
    Png = 1,
}

#[repr(C)]
pub struct AppMetadata<const N: usize> {
    magic: [u8; 8],
    pub title: [u8; 32],
    pub icon_kind: IconKind,
    pub icon_len: u32,
    pub icon: [u8; N],
}

impl<const N: usize> AppMetadata<N> {
    pub const fn new(title: &str, icon: [u8; N]) -> Self {
        assert!(!title.is_empty() && title.len() < 32);
        assert!(N <= MAX_ICON_BYTES);
        let mut name = [0; 32];
        let mut index = 0;
        while index < title.len() {
            assert!(title.as_bytes()[index] >= 32);
            name[index] = title.as_bytes()[index];
            index += 1;
        }
        Self {
            magic: MAGIC,
            title: name,
            icon_kind: if N == 0 {
                IconKind::None
            } else {
                IconKind::Png
            },
            icon_len: N as u32,
            icon,
        }
    }
}

pub struct Metadata<'a> {
    pub title: &'a str,
    pub icon: Option<&'a [u8]>,
}

fn word(bytes: &[u8], offset: usize) -> Option<usize> {
    Some(u32::from_le_bytes(bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?) as usize)
}
fn half(bytes: &[u8], offset: usize) -> Option<usize> {
    Some(u16::from_le_bytes(bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?) as usize)
}
fn range(bytes: &[u8], start: usize, len: usize) -> Option<&[u8]> {
    bytes.get(start..start.checked_add(len)?)
}

pub fn parse(bytes: &[u8]) -> Option<Metadata<'_>> {
    if bytes.get(..8)? != MAGIC {
        return None;
    }
    let title = bytes.get(8..40)?;
    let end = title.iter().position(|&byte| byte == 0)?;
    let title = core::str::from_utf8(&title[..end]).ok()?;
    if title.is_empty() || title.chars().any(char::is_control) {
        return None;
    }
    let kind = word(bytes, 40)?;
    let len = word(bytes, 44)?;
    let icon = match (kind, len) {
        (0, 0) => None,
        (1, 1..=MAX_ICON_BYTES) => Some(range(bytes, HEADER_SIZE, len)?),
        _ => return None,
    };
    Some(Metadata { title, icon })
}

pub fn from_elf(bytes: &[u8]) -> Option<Metadata<'_>> {
    if bytes.get(..6)? != b"\x7fELF\x01\x01" {
        return None;
    }
    let table = word(bytes, 32)?;
    let count = half(bytes, 48)?;
    let strings = half(bytes, 50)?;
    if half(bytes, 46)? != 40 || count > 512 || strings >= count {
        return None;
    }
    let sections = range(bytes, table, count.checked_mul(40)?)?;
    let names = &sections[strings * 40..][..40];
    let names = range(bytes, word(names, 16)?, word(names, 20)?)?;
    for section in sections.chunks_exact(40) {
        let name = names.get(word(section, 0)?..)?;
        if name.starts_with(b".tufty_app\0") {
            return parse(range(bytes, word(section, 16)?, word(section, 20)?)?);
        }
    }
    None
}

#[macro_export]
macro_rules! app_metadata {
    ($title:expr) => {
        #[used]
        #[unsafe(link_section = ".tufty_app")]
        static TUFTY_APP: $crate::app::AppMetadata<0> = $crate::app::AppMetadata::new($title, []);
    };
    ($title:expr, $icon:expr) => {
        #[used]
        #[unsafe(link_section = ".tufty_app")]
        static TUFTY_APP: $crate::app::AppMetadata<{ include_bytes!($icon).len() }> =
            $crate::app::AppMetadata::new($title, *include_bytes!($icon));
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_absence_and_invalid_discriminants() {
        let mut bytes = [0; HEADER_SIZE];
        bytes[..8].copy_from_slice(&MAGIC);
        bytes[8..12].copy_from_slice(b"Game");
        assert!(parse(&bytes).unwrap().icon.is_none());
        bytes[40] = 2;
        assert!(parse(&bytes).is_none());
        bytes[40] = 1;
        bytes[44] = 255;
        assert!(parse(&bytes).is_none());
        assert!(from_elf(&bytes).is_none());
        assert_eq!(core::mem::offset_of!(AppMetadata<0>, icon), HEADER_SIZE);
    }
}

use crate::SysError;

pub const FLASH_OFFSET: usize = 4 * 1024 * 1024;
pub const MAX_SIZE: usize = 12 * 1024 * 1024;
pub const ENTRY_SIZE: usize = 264;
pub const HEADER_SIZE: usize = 16;

#[derive(Clone, Copy)]
pub struct Pack<'a> {
    bytes: &'a [u8],
    count: usize,
}

fn word(bytes: &[u8], offset: usize) -> Result<usize, SysError> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(SysError::Io)?
            .try_into()
            .map_err(|_| SysError::Io)?,
    ) as usize)
}

impl<'a> Pack<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, SysError> {
        if bytes.get(..8) != Some(b"TUFTYPK1") {
            return Err(SysError::Io);
        }
        let count = word(bytes, 8)?;
        let length = word(bytes, 12)?;
        if count > 256 || length > MAX_SIZE || length < HEADER_SIZE + count * ENTRY_SIZE {
            return Err(SysError::Io);
        }
        let pack = Self {
            bytes: bytes.get(..length).ok_or(SysError::Io)?,
            count,
        };
        for index in 0..count {
            let (name, _) = pack.entry(index)?;
            for previous in 0..index {
                let other = pack.entry(previous)?.0;
                if name == other
                    || name
                        .strip_prefix(other)
                        .is_some_and(|rest| rest.starts_with('/'))
                    || other
                        .strip_prefix(name)
                        .is_some_and(|rest| rest.starts_with('/'))
                {
                    return Err(SysError::Io);
                }
            }
        }
        Ok(pack)
    }

    pub fn len(&self) -> usize {
        self.count
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn entry(&self, index: usize) -> Result<(&'a str, &'a [u8]), SysError> {
        if index >= self.count {
            return Err(SysError::NotFound);
        }
        let start = HEADER_SIZE + index * ENTRY_SIZE;
        let raw = &self.bytes[start..start + 256];
        let end = raw
            .iter()
            .position(|&value| value == 0)
            .ok_or(SysError::Io)?;
        let name = core::str::from_utf8(&raw[..end]).map_err(|_| SysError::Io)?;
        if name == "/" || crate::split_path(name).is_none() {
            return Err(SysError::Io);
        }
        let offset = word(self.bytes, start + 256)?;
        let size = word(self.bytes, start + 260)?;
        if offset < HEADER_SIZE + self.count * ENTRY_SIZE {
            return Err(SysError::Io);
        }
        let end = offset.checked_add(size).ok_or(SysError::Io)?;
        Ok((name, self.bytes.get(offset..end).ok_or(SysError::Io)?))
    }

    pub fn file(&self, name: &str) -> Result<&'a [u8], SysError> {
        if crate::split_path(name).is_none() {
            return Err(SysError::InvalidArgument);
        }
        for index in 0..self.count {
            let (path, bytes) = self.entry(index)?;
            if name == path {
                return Ok(bytes);
            }
        }
        Err(SysError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_and_paths_are_validated() {
        let mut bytes = [0; HEADER_SIZE + ENTRY_SIZE + 4];
        bytes[..8].copy_from_slice(b"TUFTYPK1");
        bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
        let length = bytes.len() as u32;
        bytes[12..16].copy_from_slice(&length.to_le_bytes());
        bytes[16..22].copy_from_slice(b"/a.bin");
        bytes[272..276].copy_from_slice(&280u32.to_le_bytes());
        bytes[276..280].copy_from_slice(&4u32.to_le_bytes());
        assert_eq!(Pack::parse(&bytes).unwrap().file("/a.bin"), Ok(&[0; 4][..]));
        bytes[276..280].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Pack::parse(&bytes).is_err());
        assert!(Pack::parse(&bytes[..12]).is_err());
    }
}

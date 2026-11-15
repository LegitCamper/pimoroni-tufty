//! Bounds for the badge's text-only Lua game runner.

pub const MAX_SOURCE: usize = 64 * 1024;

pub fn validate_source(source: &[u8]) -> Result<(), &'static str> {
    if source.is_empty() || source.len() > MAX_SOURCE {
        return Err("Lua script must be 1..65536 bytes");
    }
    if source.contains(&0) || source.starts_with(b"\x1bLua") {
        return Err("Lua bytecode and NUL bytes are unsupported");
    }
    core::str::from_utf8(source).map_err(|_| "Lua source must be UTF-8")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_bounded_utf8_source_is_accepted() {
        assert!(validate_source(b"function love.draw() end").is_ok());
        for source in [&b""[..], b"\x1bLua", b"text\0tail", b"\xff"] {
            assert!(validate_source(source).is_err());
        }
        let source = std::vec![b' '; MAX_SOURCE + 1];
        assert!(validate_source(&source[..MAX_SOURCE]).is_ok());
        assert!(validate_source(&source).is_err());
    }
}

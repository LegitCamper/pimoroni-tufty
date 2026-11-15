extern crate alloc;
use crate::SysError;
use alloc::vec::Vec;

pub struct Icon {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}
fn word(bytes: &[u8]) -> Result<usize, SysError> {
    Ok(u32::from_be_bytes(bytes.try_into().map_err(|_| SysError::InvalidArgument)?) as usize)
}
fn crc(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320u32 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

pub fn decode(bytes: &[u8]) -> Result<Icon, SysError> {
    if bytes.len() > crate::app::MAX_ICON_BYTES || bytes.get(..8) != Some(b"\x89PNG\r\n\x1a\n") {
        return Err(SysError::InvalidArgument);
    }
    let mut position = 8usize;
    let (mut width, mut height, mut channels) = (0, 0, 0);
    let mut compressed = Vec::new();
    let mut ended = false;
    let mut idat_ended = false;
    while position < bytes.len() {
        let header = bytes
            .get(position..position + 8)
            .ok_or(SysError::InvalidArgument)?;
        let size = word(&header[..4])?;
        let end = position
            .checked_add(12)
            .and_then(|start| start.checked_add(size))
            .ok_or(SysError::InvalidArgument)?;
        let chunk = bytes.get(position..end).ok_or(SysError::InvalidArgument)?;
        if crc(&chunk[4..8 + size]) != word(&chunk[8 + size..])? as u32 {
            return Err(SysError::InvalidArgument);
        }
        let kind = &header[4..];
        let data = &chunk[8..8 + size];
        if position == 8 && kind != b"IHDR" {
            return Err(SysError::InvalidArgument);
        }
        match kind {
            b"IHDR" => {
                if position != 8 || size != 13 {
                    return Err(SysError::InvalidArgument);
                }
                width = word(&data[..4])?;
                height = word(&data[4..8])?;
                channels = match data[9] {
                    2 => 3,
                    6 => 4,
                    _ => return Err(SysError::Unsupported),
                };
                if width == 0
                    || height == 0
                    || width > 48
                    || height > 48
                    || data[8] != 8
                    || data[10..] != [0, 0, 0]
                {
                    return Err(SysError::Unsupported);
                }
            }
            b"IDAT" => {
                if idat_ended {
                    return Err(SysError::InvalidArgument);
                }
                compressed.extend_from_slice(data);
            }
            b"IEND" => {
                if size != 0 || end != bytes.len() {
                    return Err(SysError::InvalidArgument);
                }
                ended = true;
                break;
            }
            b"PLTE" => {
                if !compressed.is_empty() || size == 0 || size > 768 || size % 3 != 0 {
                    return Err(SysError::InvalidArgument);
                }
            }
            b"tRNS" => return Err(SysError::Unsupported),
            _ if kind[0] & 32 == 0 => return Err(SysError::Unsupported),
            _ => {
                if !compressed.is_empty() {
                    idat_ended = true;
                }
            }
        }
        position = end;
    }
    if !ended || compressed.is_empty() {
        return Err(SysError::InvalidArgument);
    }
    let stride = width * channels;
    let expected = (stride + 1) * height;
    let mut raw = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&compressed, expected)
        .map_err(|_| SysError::InvalidArgument)?;
    if raw.len() != expected {
        return Err(SysError::InvalidArgument);
    }
    for row in 0..height {
        let base = row * (stride + 1);
        let filter = raw[base];
        for index in 0..stride {
            let left = if index >= channels {
                raw[base + 1 + index - channels]
            } else {
                0
            };
            let up = if row > 0 {
                raw[base + 1 + index - stride - 1]
            } else {
                0
            };
            let upper_left = if row > 0 && index >= channels {
                raw[base + index - stride - channels]
            } else {
                0
            };
            let predictor = match filter {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((left as u16 + up as u16) / 2) as u8,
                4 => {
                    let prediction = left as i16 + up as i16 - upper_left as i16;
                    let distances = [
                        (prediction - left as i16).abs(),
                        (prediction - up as i16).abs(),
                        (prediction - upper_left as i16).abs(),
                    ];
                    if distances[0] <= distances[1] && distances[0] <= distances[2] {
                        left
                    } else if distances[1] <= distances[2] {
                        up
                    } else {
                        upper_left
                    }
                }
                _ => return Err(SysError::InvalidArgument),
            };
            raw[base + 1 + index] = raw[base + 1 + index].wrapping_add(predictor);
        }
    }
    let mut rgba = Vec::with_capacity(width * height * 4);
    for row in raw.chunks_exact(stride + 1) {
        for pixel in row[1..].chunks_exact(channels) {
            rgba.extend_from_slice(&pixel[..3]);
            rgba.push(if channels == 4 { pixel[3] } else { 255 });
        }
    }
    Ok(Icon {
        width,
        height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], bytes: &[u8]) {
        png.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        let start = png.len();
        png.extend_from_slice(kind);
        png.extend_from_slice(bytes);
        let checksum = crc(&png[start..]);
        png.extend_from_slice(&checksum.to_be_bytes());
    }
    #[test]
    fn reconstructs_all_filters_and_preserves_alpha() {
        for filter in 0..5 {
            let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
            chunk(&mut png, b"IHDR", &[0, 0, 0, 2, 0, 0, 0, 2, 8, 6, 0, 0, 0]);
            let pixel = [20u8, 40, 80, 120];
            let mut raw = Vec::new();
            for row in 0..2 {
                raw.push(filter);
                for column in 0..2 {
                    for value in pixel {
                        let left = if column > 0 { value } else { 0 };
                        let up = if row > 0 { value } else { 0 };
                        let prediction = match filter {
                            0 => 0,
                            1 => left,
                            2 => up,
                            3 => ((left as u16 + up as u16) / 2) as u8,
                            4 => left.max(up),
                            _ => unreachable!(),
                        };
                        raw.push(value.wrapping_sub(prediction));
                    }
                }
            }
            chunk(
                &mut png,
                b"IDAT",
                &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6),
            );
            chunk(&mut png, b"IEND", &[]);
            assert_eq!(decode(&png).unwrap().rgba, pixel.repeat(4));
        }
    }
    #[test]
    fn rejects_truncated_and_corrupt_images() {
        assert!(decode(b"\x89PNG\r\n\x1a\n").is_err());
        let icon = include_bytes!("../../assets/snake.png");
        let decoded = decode(icon).unwrap();
        assert_eq!((decoded.width, decoded.height), (32, 32));
        let mut broken = icon.to_vec();
        broken[20] ^= 1;
        assert!(decode(&broken).is_err());
    }
}

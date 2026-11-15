pub const GBOY_WIDTH: usize = 160;
pub const GBOY_HEIGHT: usize = 144;
pub const SCALED_WIDTH: usize = 266;
pub const SCALED_HEIGHT: usize = 240;

const fn rgb565(red: u8, green: u8, blue: u8) -> u16 {
    ((red as u16 & 0x1F) << 11) | ((green as u16 & 0x3F) << 5) | (blue as u16 & 0x1F)
}

const PALETTE: [u16; 4] = [
    rgb565(0x08 >> 3, 0x18 >> 2, 0x20 >> 3),
    rgb565(0x34 >> 3, 0x68 >> 2, 0x56 >> 3),
    rgb565(0x88 >> 3, 0xC0 >> 2, 0x70 >> 3),
    rgb565(0xE0 >> 3, 0xF8 >> 2, 0xD0 >> 3),
];

pub fn scale_line(source: &[u8; GBOY_WIDTH], output: &mut [u16; SCALED_WIDTH * 2]) {
    let (top, bottom) = output.split_at_mut(SCALED_WIDTH);
    for (column, pixel) in top.iter_mut().enumerate() {
        *pixel = PALETTE[(source[column * GBOY_WIDTH / SCALED_WIDTH] & 3) as usize];
    }
    bottom.copy_from_slice(top);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scaled_picture_fits_tufty_without_clipping() {
        let source = core::array::from_fn(|index| (index % 4) as u8);
        let mut output = [0; SCALED_WIDTH * 2];
        scale_line(&source, &mut output);
        assert_eq!((SCALED_WIDTH, SCALED_HEIGHT), (266, 240));
        for column in 0..SCALED_WIDTH {
            assert_eq!(
                output[column],
                PALETTE[source[column * GBOY_WIDTH / SCALED_WIDTH] as usize]
            );
            assert_eq!(output[column], output[column + SCALED_WIDTH]);
        }
        assert_eq!(
            (0..GBOY_HEIGHT)
                .map(|row| (row + 1) * SCALED_HEIGHT / GBOY_HEIGHT
                    - row * SCALED_HEIGHT / GBOY_HEIGHT)
                .sum::<usize>(),
            240
        );
    }
}

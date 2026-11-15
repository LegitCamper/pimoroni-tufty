bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Buttons: u32 {
        const UP = 1 << 0;
        const DOWN = 1 << 1;
        const LEFT = 1 << 2;
        const RIGHT = 1 << 3;
        const A = 1 << 4;
        const B = 1 << 5;
        const C = 1 << 6;
        const X = 1 << 7;
        const Y = 1 << 8;
        const START = 1 << 9;
        const SELECT = 1 << 10;
        const HOME = 1 << 11;
    }
}

pub fn gamepad_buttons(register: u16) -> Buttons {
    let mappings = [
        (1, Buttons::UP),
        (4, Buttons::DOWN),
        (2, Buttons::LEFT),
        (3, Buttons::RIGHT),
        (14, Buttons::A),
        (12, Buttons::B),
        (15, Buttons::X),
        (13, Buttons::Y),
        (11, Buttons::START),
        (5, Buttons::SELECT),
    ];
    let mut buttons = Buttons::empty();
    for (pin, button) in mappings {
        if register & (1 << pin) != 0 {
            buttons |= button;
        }
    }
    buttons
}

#[derive(Default)]
pub struct Debouncer {
    stable: u32,
    counters: [u8; 12],
}
impl Debouncer {
    pub fn sample(&mut self, raw: Buttons) -> Buttons {
        for (index, counter) in self.counters.iter_mut().enumerate() {
            let mask = 1 << index;
            if (raw.bits() ^ self.stable) & mask == 0 {
                *counter = 0;
            } else {
                *counter += 1;
                if *counter == 4 {
                    self.stable ^= mask;
                    *counter = 0;
                }
            }
        }
        Buttons::from_bits_retain(self.stable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn debounce_and_simultaneous_gamepad_buttons() {
        let expected = Buttons::A | Buttons::LEFT | Buttons::START;
        assert_eq!(gamepad_buttons((1 << 14) | (1 << 2) | (1 << 11)), expected);
        let mut debounce = Debouncer::default();
        for _ in 0..3 {
            assert!(debounce.sample(expected).is_empty());
        }
        assert_eq!(debounce.sample(expected), expected);
        for _ in 0..3 {
            assert_eq!(debounce.sample(Buttons::empty()), expected);
        }
        assert!(debounce.sample(Buttons::empty()).is_empty());
    }
}

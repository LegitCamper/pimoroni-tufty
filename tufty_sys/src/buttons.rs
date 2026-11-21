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
        const BADGE_A = 1 << 12;
        const BADGE_B = 1 << 13;
        const BADGE_C = Self::C.bits();
    }
}

pub const BADGE_BUTTONS: [Buttons; 6] = [
    Buttons::UP,
    Buttons::DOWN,
    Buttons::BADGE_A,
    Buttons::BADGE_B,
    Buttons::BADGE_C,
    Buttons::HOME,
];

pub fn app_buttons(badge: Buttons, gamepad: Buttons) -> Buttons {
    (badge - (Buttons::UP | Buttons::DOWN)) | gamepad
}

pub fn adjust_brightness(level: u8, current: Buttons, previous: Buttons) -> u8 {
    let level = level.clamp(10, 100);
    if current.contains(Buttons::UP | Buttons::DOWN) {
        return level;
    }
    let pressed = current - previous;
    if pressed.contains(Buttons::UP) {
        level.saturating_add(10).min(100)
    } else if pressed.contains(Buttons::DOWN) {
        level.saturating_sub(10).max(10)
    } else {
        level
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
pub struct ResetHold {
    started_ms: Option<u64>,
}

impl ResetHold {
    pub fn sample(&mut self, pressed: bool, now_ms: u64) -> bool {
        if !pressed {
            self.started_ms = None;
            return false;
        }
        let started_ms = *self.started_ms.get_or_insert(now_ms);
        now_ms.saturating_sub(started_ms) >= 2_000
    }
}

#[derive(Default)]
pub struct Debouncer {
    stable: u32,
    counters: [u8; 14],
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
    fn gamepad_plus_minus_reach_apps_as_start_select() {
        for (register, expected) in [
            (1 << 11, Buttons::START),
            (1 << 5, Buttons::SELECT),
            ((1 << 11) | (1 << 5), Buttons::START | Buttons::SELECT),
        ] {
            let pad = gamepad_buttons(register);
            assert_eq!(pad, expected);
            let mut debounce = Debouncer::default();
            for _ in 0..3 {
                assert!(debounce.sample(pad).is_empty());
            }
            assert_eq!(
                app_buttons(Buttons::empty(), debounce.sample(pad)),
                expected
            );
            assert_eq!(
                app_buttons(Buttons::UP | Buttons::DOWN | Buttons::BADGE_A, pad),
                expected | Buttons::BADGE_A,
            );
            for _ in 0..3 {
                assert_eq!(debounce.sample(Buttons::empty()), expected);
            }
            assert!(debounce.sample(Buttons::empty()).is_empty());
        }
    }

    #[test]
    fn badge_buttons_are_distinct_and_brightness_does_not_reach_apps() {
        let badge = BADGE_BUTTONS
            .into_iter()
            .fold(Buttons::empty(), |all, button| all | button);
        let actions = Buttons::BADGE_A | Buttons::BADGE_B | Buttons::BADGE_C;
        assert_eq!(
            app_buttons(badge, Buttons::empty()),
            actions | Buttons::HOME
        );
        assert!((actions & (Buttons::A | Buttons::B)).is_empty());
        assert_eq!(Buttons::BADGE_C, Buttons::C);
        let pad = gamepad_buttons((1 << 14) | (1 << 12) | (1 << 1));
        assert_eq!(
            app_buttons(badge, pad),
            actions | Buttons::HOME | Buttons::A | Buttons::B | Buttons::UP
        );
        assert_eq!(app_buttons(Buttons::empty(), pad), pad);
        assert_eq!(app_buttons(actions, Buttons::empty()), actions);
    }

    #[test]
    fn debounce_preserves_all_badge_actions_and_home() {
        let actions = Buttons::BADGE_A | Buttons::BADGE_B | Buttons::BADGE_C | Buttons::HOME;
        let mut debounce = Debouncer::default();
        for _ in 0..3 {
            assert!(debounce.sample(actions).is_empty());
        }
        assert_eq!(debounce.sample(actions), actions);
        for _ in 0..3 {
            assert_eq!(debounce.sample(Buttons::BADGE_B), actions);
        }
        assert_eq!(debounce.sample(Buttons::BADGE_B), Buttons::BADGE_B);
    }

    #[test]
    fn brightness_steps_on_press_and_stays_visible() {
        assert_eq!(adjust_brightness(50, Buttons::UP, Buttons::empty()), 60);
        assert_eq!(adjust_brightness(50, Buttons::DOWN, Buttons::empty()), 40);
        assert_eq!(adjust_brightness(50, Buttons::UP, Buttons::UP), 50);
        assert_eq!(adjust_brightness(50, Buttons::empty(), Buttons::DOWN), 50);
        assert_eq!(
            adjust_brightness(50, Buttons::UP | Buttons::DOWN, Buttons::DOWN),
            50
        );
        assert_eq!(adjust_brightness(100, Buttons::UP, Buttons::empty()), 100);
        assert_eq!(adjust_brightness(10, Buttons::DOWN, Buttons::empty()), 10);
        assert_eq!(adjust_brightness(0, Buttons::empty(), Buttons::empty()), 10);
        assert_eq!(
            adjust_brightness(255, Buttons::empty(), Buttons::empty()),
            100
        );
        assert_eq!(
            adjust_brightness(
                50,
                Buttons::BADGE_A | Buttons::BADGE_B | Buttons::BADGE_C,
                Buttons::empty()
            ),
            50
        );
    }

    #[test]
    fn reset_requires_two_seconds_of_continuous_hold() {
        let mut hold = ResetHold::default();
        assert!(!hold.sample(false, 0));
        assert!(!hold.sample(true, 100));
        assert!(!hold.sample(true, 2_099));
        assert!(hold.sample(true, 2_100));
        assert!(!hold.sample(false, 2_101));
        assert!(!hold.sample(true, 5_000));
        assert!(!hold.sample(true, 6_999));
        assert!(hold.sample(true, 7_000));
        assert!(!hold.sample(true, 4_000));
    }

    #[test]
    fn short_reset_holds_do_not_accumulate() {
        let mut hold = ResetHold::default();
        for start in [0, 2_000, 4_000] {
            assert!(!hold.sample(true, start));
            assert!(!hold.sample(true, start + 1_500));
            assert!(!hold.sample(false, start + 1_999));
        }
    }

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

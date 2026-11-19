"""Source-order guards for LCD startup; electrical behavior needs badge testing."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class DisplayStartupTests(unittest.TestCase):
    def test_backlight_is_disabled_before_reset_hold_and_power_enable(self):
        source = (ROOT / "kernel/src/main.rs").read_text()
        disabled = source.index("let backlight = Output::new(peripherals.PIN_26, Level::Low);")
        self.assertLess(disabled, source.index("power::check_reset("))
        self.assertLess(disabled, source.index("Output::new(peripherals.PIN_41, Level::High)"))
        self.assertIn("            backlight,", source)

    def test_complete_black_frame_precedes_display_and_backlight_enable(self):
        source = (ROOT / "kernel/src/display.rs").read_text()
        steps = [
            "panel.command(0x2b, &[0, 0, 1, 63]).await;",
            "let mut line = [0u8; SCREEN_HEIGHT * 2];",
            "panel.command(0x2c, &[]).await;",
            "panel.cs.set_low();",
            "panel.dc.set_high();",
            "for _ in 0..SCREEN_WIDTH {\n        panel.write(&line).await;\n    }",
            "panel.cs.set_high();",
            "panel.command(0x29, &[]).await;",
            "Timer::after_millis(20).await;",
            "panel.backlight.set_high();",
            "*FRAMEBUFFER.lock().await = Some(FrameBuffer {",
            "dirty: false,",
        ]
        position = source.index("pub async fn display_task")
        for step in steps:
            with self.subTest(step=step):
                position = source.index(step, position) + len(step)
        self.assertEqual(source.count("panel.backlight.set_high();"), 1)


if __name__ == "__main__":
    unittest.main()

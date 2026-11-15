"""Run original tiny cartridge programs through the actual vendored CPU/PPU cores."""
from pathlib import Path
import os
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1] / "user_apps/consoles"


class ConsoleTests(unittest.TestCase):
    def test_native_frames_and_repeated_sessions_under_asan(self):
        sources = [ROOT / "native" / f"{name}.c" for name in
                   ("smoke", "nes", "snes", "gbc_checks", "gba", "portable")]
        sources += [ROOT / "vendor/lakesnes" / f"{name}.c" for name in
                    ("snes", "cpu", "apu", "spc", "dsp", "dma", "ppu", "cart", "input")]
        with tempfile.TemporaryDirectory(prefix="tufty-cores-") as directory:
            executable = Path(directory) / "smoke"
            compiled = subprocess.run([os.environ.get("CC", "cc"), "-std=gnu11", "-O1", "-g",
                            "-DNDEBUG", "-fno-strict-aliasing", "-fwrapv", "-fno-omit-frame-pointer",
                            "-ffunction-sections", "-fdata-sections", "-fsanitize=address",
                            "-Wl,--gc-sections", "-I", str(ROOT / "native"),
                            *map(str, sources), "-o", str(executable)],
                           check=False, timeout=120, capture_output=True, text=True)
            self.assertEqual(compiled.returncode, 0, compiled.stdout + compiled.stderr)
            result = subprocess.run([str(executable)], check=False, timeout=30,
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            for name in ("NES", "SNES", "GBC", "GBA"):
                self.assertEqual(sum(line.startswith(f"{name}:") for line in result.stdout.splitlines()), 3, result.stdout)


if __name__ == "__main__":
    unittest.main()

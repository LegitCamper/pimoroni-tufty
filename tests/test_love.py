"""Exercise the actual badge Lua adapter with sanitizers and original scripts."""
from pathlib import Path
import os
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1] / "user_apps/love"


class LoveTests(unittest.TestCase):
    def test_lua_runtime(self):
        sources = [ROOT / "native/runner.c", ROOT / "native/check.c", ROOT / "native/pico8_check.c"]
        sources += [ROOT / "vendor/lua" / f"{name}.c"
                    for name in (ROOT / "native/sources.txt").read_text().split()]
        with tempfile.TemporaryDirectory(prefix="tufty-lua-") as directory:
            executable = Path(directory) / "check"
            result = subprocess.run([
                os.environ.get("CC", "cc"), "-std=c11", "-O1", "-g",
                "-fno-omit-frame-pointer", "-fsanitize=address,undefined",
                "-ffunction-sections", "-fdata-sections", "-Wl,--gc-sections",
                "-include", str(ROOT / "native/config.h"),
                "-I", str(ROOT / "vendor/lua"), *map(str, sources),
                "-lm", "-o", str(executable)
            ], capture_output=True, text=True, timeout=120)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            pico_demo = Path(directory) / "pico.lua"
            pico_demo.write_text((ROOT / "demo.p8").read_text().split("__lua__\n", 1)[1])
            result = subprocess.run([str(executable), str(ROOT / "demo.lua"), str(pico_demo)],
                                    capture_output=True, text=True, timeout=30,
                                    env={**os.environ, "UBSAN_OPTIONS": "halt_on_error=1"})
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("repeated sessions passed", result.stdout)
            self.assertIn("P8 API, palettes", result.stdout)

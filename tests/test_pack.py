import importlib.util
from pathlib import Path
import struct
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("pack", Path(__file__).parents[1] / "tools/pack.py")
pack = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pack)


class PackTests(unittest.TestCase):
    def test_offsets_and_determinism(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "apps").mkdir()
            (root / "apps/game.bin").write_bytes(b"ELF")
            data = pack.build_pack(root)
            self.assertEqual(data, pack.build_pack(root))
            self.assertEqual(struct.unpack_from("<II", data, 8), (1, 283))
            self.assertEqual(struct.unpack_from("<II", data, 272), (280, 3))
            self.assertEqual(data[280:], b"ELF")
            (root / "link").symlink_to(root / "apps/game.bin")
            with self.assertRaises(ValueError):
                pack.build_pack(root)

    def test_uf2_checks_and_block_numbers(self):
        kernel = bytearray(52 + 32 + 256)
        kernel[:7] = b"\x7fELF\x01\x01\x01"
        struct.pack_into("<HHIII", kernel, 16, 2, 40, 1, 0x10000001, 52)
        struct.pack_into("<HHH", kernel, 40, 52, 32, 1)
        struct.pack_into("<8I", kernel, 52, 1, 84, 0x10000000, 0x10000000, 256, 256, 5, 4)
        output = pack.combine(kernel, b"test")
        self.assertEqual(len(output), 1024)
        self.assertEqual(struct.unpack_from("<III", output, 512 + 12), (pack.FLASH_ADDRESS, 256, 1))
        with self.assertRaises(ValueError):
            pack.combine(kernel[:-1], b"test")
        self.assertEqual(struct.unpack_from("<I", output, 28)[0], pack.FAMILY)
        struct.pack_into("<I", kernel, 52 + 12, pack.FLASH_ADDRESS)
        with self.assertRaises(ValueError):
            pack.combine(kernel, b"test")


if __name__ == "__main__":
    unittest.main()

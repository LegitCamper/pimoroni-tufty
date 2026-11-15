"""Generate original 32px RGB app icons; Python standard library only."""
from pathlib import Path
import struct
import zlib


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


def icon(name):
    pixels = bytearray()
    for row in range(32):
        pixels.append(0)
        for column in range(32):
            active = False
            if name == "snake":
                active = (6 <= column < 12 and 7 <= row < 26) or (6 <= column < 24 and 20 <= row < 26) or (18 <= column < 24 and 12 <= row < 26)
            elif name == "gboy":
                active = (6 <= column < 26 and 3 <= row < 29) and not (10 <= column < 22 and 7 <= row < 17)
            elif name == "chip8":
                active = (6 <= column < 26 and 6 <= row < 26) and ((row // 4 + column // 4) % 2 == 0)
            pixels.extend((255, 193, 56) if active else (24, 48, 64))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 32, 32, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b"")


if __name__ == "__main__":
    destination = Path(__file__).resolve().parent.parent / "assets"
    destination.mkdir(exist_ok=True)
    for name in ("snake", "gboy", "chip8"):
        (destination / f"{name}.png").write_bytes(icon(name))

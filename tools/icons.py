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
            elif name in ("gboy", "gbc"):
                active = (6 <= column < 26 and 3 <= row < 29) and not (10 <= column < 22 and 7 <= row < 17)
            elif name == "gba":
                active = (2 <= column < 30 and 8 <= row < 25) and not (9 <= column < 23 and 11 <= row < 21)
            elif name == "nes":
                active = (3 <= column < 29 and 9 <= row < 24) and not (7 <= column < 13 and 13 <= row < 20) and not (20 <= column < 26 and 16 <= row < 20)
            elif name == "snes":
                active = (3 <= column < 29 and 7 <= row < 26) and not (6 <= column < 13 and 13 <= row < 20) and not (19 <= column < 26 and 12 <= row < 21)
            elif name == "chip8":
                active = (6 <= column < 26 and 6 <= row < 26) and ((row // 4 + column // 4) % 2 == 0)
            elif name == "love":
                active = ((column - 11) ** 2 + (row - 11) ** 2 < 36 or (column - 21) ** 2 + (row - 11) ** 2 < 36 or (10 <= row < 27 and abs(column - 16) < (28 - row))) and 4 <= column < 28
            elif name == "pico8":
                active = (5 <= column < 27 and 3 <= row < 29) and not (9 <= column < 23 and 7 <= row < 18) or (13 <= column < 19 and 10 <= row < 15)
            color = {"nes": (255, 105, 90), "snes": (110, 180, 255), "gbc": (120, 240, 160), "gba": (255, 180, 80)}.get(name, (255, 193, 56))
            pixels.extend(color if active else (24, 48, 64))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 32, 32, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b"")


if __name__ == "__main__":
    destination = Path(__file__).resolve().parent.parent / "assets"
    destination.mkdir(exist_ok=True)
    for name in ("snake", "gboy", "chip8", "nes", "snes", "gbc", "gba", "love", "pico8"):
        (destination / f"{name}.png").write_bytes(icon(name))

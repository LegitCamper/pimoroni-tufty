"""Build immutable Tufty flash packs and RP2350 UF2 from a kernel ELF."""
import argparse
from pathlib import Path
import struct
import tempfile

FLASH_ADDRESS = 0x10400000
MAX_SIZE = 12 * 1024 * 1024
FAMILY = 0xE48BFF59
ENTRY_SIZE = 264


def build_pack(root):
    if not root.is_dir():
        raise ValueError("pack source must be a directory")
    files = []
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise ValueError(f"symlink rejected: {path}")
        if not path.is_file():
            continue
        name = "/" + path.relative_to(root).as_posix()
        parts = name[1:].split("/")
        if len(name.encode()) >= 256 or len(parts) > 8 or any(part in ("", ".", "..") or part.endswith((" ", ".")) or any(ord(char) < 32 or char in '\\:*?"<>|\uffff' for char in part) for part in parts):
            raise ValueError(f"invalid pack path: {name}")
        if path.stat().st_size > MAX_SIZE:
            raise ValueError(f"file exceeds flash partition: {name}")
        files.append((name, path))
    if len(files) > 256:
        raise ValueError("pack supports at most 256 files")
    offset = 16 + len(files) * ENTRY_SIZE
    entries = bytearray()
    data = bytearray()
    for name, path in files:
        content = path.read_bytes()
        if offset + len(content) > MAX_SIZE:
            raise ValueError("pack exceeds 12 MiB flash partition")
        entries.extend(name.encode().ljust(256, b"\0"))
        entries.extend(struct.pack("<II", offset, len(content)))
        data.extend(content)
        offset += len(content)
    return b"TUFTYPK1" + struct.pack("<II", len(files), offset) + entries + data


def combine(kernel, pack):
    if len(kernel) < 52 or kernel[:7] != b"\x7fELF\x01\x01\x01":
        raise ValueError("kernel must be little-endian ELF32")
    kind, machine, version, entry, table = struct.unpack_from("<HHIII", kernel, 16)
    header_size, stride, count = struct.unpack_from("<HHH", kernel, 40)
    if (kind, machine, version, header_size, stride) != (2, 40, 1, 52, 32) or count == 0 or count > 128 or table + stride * count > len(kernel) or not 0x10000000 <= (entry & ~1) < FLASH_ADDRESS or not entry & 1:
        raise ValueError("invalid ARM executable header")
    pages = {}
    ranges = []
    for index in range(count):
        kind, offset, _, address, size, memory_size, _, _ = struct.unpack_from("<8I", kernel, table + index * stride)
        if kind != 1 or size == 0:
            continue
        end = address + size
        if size > memory_size or offset + size > len(kernel) or not 0x10000000 <= address < end <= FLASH_ADDRESS:
            raise ValueError("load segment outside kernel flash partition")
        if any(address < stop and start < end for start, stop in ranges):
            raise ValueError("overlapping ELF load segments")
        ranges.append((address, end))
        consumed = 0
        while consumed < size:
            location = address + consumed
            page = pages.setdefault(location & ~255, bytearray(b"\xff" * 256))
            start = location & 255
            amount = min(256 - start, size - consumed)
            page[start:start + amount] = kernel[offset + consumed:offset + consumed + amount]
            consumed += amount
    if 0x10000000 not in pages or not any(start <= (entry & ~1) < end for start, end in ranges):
        raise ValueError("missing flash vector table or entry")
    blocks = sorted(pages.items())
    if not pack or len(pack) > MAX_SIZE:
        raise ValueError("invalid pack size")
    for offset in range(0, len(pack), 256):
        blocks.append((FLASH_ADDRESS + offset, pack[offset:offset + 256].ljust(256, b"\xff")))
    output = bytearray()
    for index, (address, payload) in enumerate(blocks):
        header = struct.pack("<8I", 0x0A324655, 0x9E5D5157, 0x2000, address, 256, index, len(blocks), FAMILY)
        output.extend(header + payload + bytes(220) + struct.pack("<I", 0x0AB16F30))
    return output


def atomic_write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as output:
            temporary = Path(output.name)
            output.write(data)
        temporary.replace(path)
    finally:
        if temporary and temporary.exists():
            temporary.unlink()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--kernel", type=Path, help="combine kernel ELF and pack into bootable UF2")
    arguments = parser.parse_args()
    try:
        data = build_pack(arguments.root)
        if arguments.kernel:
            data = combine(arguments.kernel.read_bytes(), data)
        atomic_write(arguments.output, data)
        print(f"{arguments.output}: {len(data)} bytes")
    except (ValueError, OSError) as error:
        parser.exit(1, f"pack: {error}\n")

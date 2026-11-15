"""Fetch pinned, hash-verified CYW43439 firmware for the optional wifi feature."""
import hashlib
from pathlib import Path
import urllib.request
from pack import atomic_write

COMMIT = "49c861fbdadebbc2ebab253d356ce4b2e3bc67ad"
FILES = {
    "43439A0.bin": "4171a0906cad80c7ee6398a80233b08f0089b65c021ecb9ffed33f02b6ed3c5b",
    "43439A0_clm.bin": "27f9abd62bd92858d54e4825f791e27f65b6a4ce94eafc3c729d4760ce507762",
}

if __name__ == "__main__":
    directory = Path(__file__).resolve().parent.parent / "firmware"
    for name, digest in FILES.items():
        path = directory / name
        if path.exists() and hashlib.sha256(path.read_bytes()).hexdigest() == digest:
            continue
        url = f"https://raw.githubusercontent.com/embassy-rs/cyw43/{COMMIT}/firmware/{name}"
        with urllib.request.urlopen(url, timeout=30) as response:
            data = response.read(512 * 1024)
        if hashlib.sha256(data).hexdigest() != digest:
            raise SystemExit(f"checksum mismatch: {name}; existing file unchanged")
        atomic_write(path, data)
        print(f"verified {name}")

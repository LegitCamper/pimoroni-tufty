# Pinned emulator sources

All cores are MIT licensed; copyright and full license notices remain in `vendor`.
No desktop frontends, SDL, commercial cartridges or proprietary BIOS files are included.

| Core | Upstream | Commit | Imported subset |
| --- | --- | --- | --- |
| agnes | https://github.com/kgabis/agnes | `0e4220b084c467e39c04805d955e78c463feadd0` | `agnes.c`, `agnes.h`, `LICENSE` |
| LakeSnes | https://github.com/angelo-wf/LakeSnes | `9db90b86e46a377609305e298dd92d71cd1d4c8a` | `snes/*.[ch]`, `LICENSE.txt` |
| SkyEmu | https://github.com/skylersaleh/SkyEmu | `b64903f0fde8104d0796546b96859e9f682311b7` | GB/GBA/ARM7 headers, instruction tables, shared types, replacement GBA BIOS, `LICENSE` |
| Cult-of-GBA BIOS | https://github.com/Cult-of-GBA/BIOS | `0d9dd3e6283896eab340522d51234799d5c46626` | MIT replacement BIOS embedded by SkyEmu; full license in `vendor/skyemu/BIOS-LICENSE` |

## Local integration changes

- agnes: bounded cartridge-byte reads, `memcmp` for fixed-length magic. Native
  adapter initializes context in caller-owned storage instead of `malloc`.
- LakeSnes: unchanged core source. Adapter wires CPU/APU/PPU/DMA/controllers into
  one owned session, aliases the validated ROM without making duplicate copies,
  provides bounded battery RAM, and reads the PPU buffer without a 960 KiB copy.
  Only plain power-of-two LoROM/HiROM is admitted, not enhancement chips.
- SkyEmu: silent builds retain guest sound timers/FIFOs but skip host sample
  synthesis. Integer power-of-two noise calculation replaces `pow`. Host files,
  debug logging and BIOS searches are disabled by the native portability header.
  GB skips the boot ROM; GBA uses the MIT replacement BIOS, not Nintendo code.
  Small GB RAM banks avoid a zero divisor; MBC2 uses 512 nibble cells and address
  bit 8 to distinguish bank selection from RAM enable.
- ARM7 decode tables belong to the GBA session, not permanent kernel SRAM.
  Unknown opcode table entries use the upstream undefined-instruction handler.
  GBA frames have an instruction budget so invalid execution can return to UI.
- RTC starts at 2000-01-01 UTC plus badge uptime. No wall-clock synchronization or
  persistent clock/save state is claimed. GB RTC behavior otherwise follows SkyEmu.

ROM ownership stays in Rust `PsramBuffer` allocations. NES state uses a fallible
`Vec<u64>` in the runtime SRAM arena; larger cores use `PsramBuffer`. Sessions
contain no atomics and have 8-byte alignment. No native allocator or file callbacks
cross into Rust. All sessions are dropped on return. The cores remain trusted
native code, not a sandbox for hostile ROM programs.

`tests/test_consoles.py` compiles these exact sources with AddressSanitizer and
executes original tiny cartridge programs on all four cores, checking pixels,
row-buffer guards and repeated session allocation/release. This is not a console
compatibility suite or a badge performance measurement.

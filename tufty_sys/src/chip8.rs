#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Chip8,
    SuperChip,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    RomSize,
    Address,
    Stack,
    Opcode,
}

pub struct Machine {
    memory: [u8; 4096],
    registers: [u8; 16],
    stack: [u16; 16],
    stack_len: usize,
    index: u16,
    pc: u16,
    flags: [u8; 8],
    random: u32,
    mode: Mode,
    pub pixels: [u8; 128 * 64],
    pub width: usize,
    pub height: usize,
    pub delay: u8,
    pub sound: u8,
    pub halted: bool,
    pub dirty: bool,
}

const FONT: [u8; 80] = [
    0xf0, 0x90, 0x90, 0x90, 0xf0, 0x20, 0x60, 0x20, 0x20, 0x70, 0xf0, 0x10, 0xf0, 0x80, 0xf0, 0xf0,
    0x10, 0xf0, 0x10, 0xf0, 0x90, 0x90, 0xf0, 0x10, 0x10, 0xf0, 0x80, 0xf0, 0x10, 0xf0, 0xf0, 0x80,
    0xf0, 0x90, 0xf0, 0xf0, 0x10, 0x20, 0x40, 0x40, 0xf0, 0x90, 0xf0, 0x90, 0xf0, 0xf0, 0x90, 0xf0,
    0x10, 0xf0, 0xf0, 0x90, 0xf0, 0x90, 0x90, 0xe0, 0x90, 0xe0, 0x90, 0xe0, 0xf0, 0x80, 0x80, 0x80,
    0xf0, 0xe0, 0x90, 0x90, 0x90, 0xe0, 0xf0, 0x80, 0xf0, 0x80, 0xf0, 0xf0, 0x80, 0xf0, 0x80, 0x80,
];

pub const DEMO: [u8; 33] = [
    0x60, 0x00, 0x61, 0x10, 0xa2, 0x1e, 0x00, 0xe0, 0xd0, 0x13, 0x62, 0x02, 0xf2, 0x15, 0xf2, 0x07,
    0x32, 0x00, 0x12, 0x0e, 0x70, 0x01, 0x30, 0x40, 0x12, 0x06, 0x60, 0x00, 0x12, 0x06, 0xf0, 0x90,
    0xf0,
];

impl Machine {
    pub fn new(rom: &[u8], mode: Mode, seed: u32) -> Result<Self, Error> {
        let mut storage = core::mem::MaybeUninit::uninit();
        Self::init(&mut storage, rom, mode, seed)?;
        // SAFETY: init succeeded and initialized every field.
        Ok(unsafe { storage.assume_init() })
    }

    /// Initializes caller-owned storage without copying a large machine through the stack.
    pub fn init<'storage>(
        storage: &'storage mut core::mem::MaybeUninit<Self>,
        rom: &[u8],
        mode: Mode,
        seed: u32,
    ) -> Result<&'storage mut Self, Error> {
        if rom.is_empty() || rom.len() > 4096 - 512 {
            return Err(Error::RomSize);
        }
        let pointer = storage.as_mut_ptr();
        // SAFETY: storage is exclusive and sized for Self. All fields except Mode admit
        // zero bits; Mode is explicitly written before creating any initialized reference.
        let machine = unsafe {
            pointer.write_bytes(0, 1);
            core::ptr::addr_of_mut!((*pointer).mode).write(mode);
            storage.assume_init_mut()
        };
        machine.pc = 512;
        machine.random = seed.max(1);
        machine.width = 64;
        machine.height = 32;
        machine.dirty = true;
        machine.memory[0x50..0xa0].copy_from_slice(&FONT);
        for (row, &small) in FONT.iter().enumerate() {
            let mut large = 0;
            for column in 0..4 {
                if small & (0x80 >> column) != 0 {
                    large |= 0xc0 >> (column * 2);
                }
            }
            machine.memory[0xa0 + row * 2..0xa0 + row * 2 + 2].fill(large);
        }
        machine.memory[512..512 + rom.len()].copy_from_slice(rom);
        Ok(machine)
    }
    pub fn tick(&mut self) {
        self.delay = self.delay.saturating_sub(1);
        self.sound = self.sound.saturating_sub(1);
    }
    fn range(&self, count: usize) -> Result<core::ops::Range<usize>, Error> {
        let start = self.index as usize;
        let end = start + count;
        if end > self.memory.len() {
            Err(Error::Address)
        } else {
            Ok(start..end)
        }
    }
    fn scroll(&mut self, right: i32, down: i32) {
        for row in (0..self.height).rev() {
            for step in 0..self.width {
                let column = if right > 0 {
                    self.width - 1 - step
                } else {
                    step
                };
                let source_x = column as i32 - right;
                let source_y = row as i32 - down;
                self.pixels[row * self.width + column] =
                    if source_x >= 0 && source_x < self.width as i32 && source_y >= 0 {
                        self.pixels[source_y as usize * self.width + source_x as usize]
                    } else {
                        0
                    };
            }
        }
        self.dirty = true;
    }
    pub fn step(&mut self, keys: u16, pressed: u16) -> Result<(), Error> {
        if self.halted {
            return Ok(());
        }
        let pc = self.pc as usize;
        let bytes = self.memory.get(pc..pc + 2).ok_or(Error::Address)?;
        let opcode = u16::from_be_bytes([bytes[0], bytes[1]]);
        self.pc += 2;
        let target = opcode & 0xfff;
        let first = ((opcode >> 8) & 15) as usize;
        let second = ((opcode >> 4) & 15) as usize;
        let nibble = (opcode & 15) as usize;
        let immediate = opcode as u8;
        let left = self.registers[first];
        let right = self.registers[second];
        let superchip = self.mode == Mode::SuperChip;
        match opcode >> 12 {
            0 => match opcode {
                0x00e0 => {
                    self.pixels.fill(0);
                    self.dirty = true;
                }
                0x00ee => {
                    if self.stack_len == 0 {
                        return Err(Error::Stack);
                    }
                    self.stack_len -= 1;
                    self.pc = self.stack[self.stack_len];
                }
                0x00fd if superchip => self.halted = true,
                0x00fe | 0x00ff if superchip => {
                    self.width = if opcode == 0xff { 128 } else { 64 };
                    self.height = self.width / 2;
                    self.pixels.fill(0);
                    self.dirty = true;
                }
                0x00fb if superchip => self.scroll(4, 0),
                0x00fc if superchip => self.scroll(-4, 0),
                _ if superchip && opcode & 0xfff0 == 0x00c0 => self.scroll(0, nibble as i32),
                _ => return Err(Error::Opcode),
            },
            1 => self.pc = target,
            2 => {
                if self.stack_len == 16 {
                    return Err(Error::Stack);
                }
                self.stack[self.stack_len] = self.pc;
                self.stack_len += 1;
                self.pc = target;
            }
            3 => {
                if left == immediate {
                    self.pc += 2;
                }
            }
            4 => {
                if left != immediate {
                    self.pc += 2;
                }
            }
            5 if nibble == 0 => {
                if left == right {
                    self.pc += 2;
                }
            }
            6 => self.registers[first] = immediate,
            7 => self.registers[first] = left.wrapping_add(immediate),
            8 => {
                let operand = if superchip { left } else { right };
                let (value, flag) = match nibble {
                    0 => (right, None),
                    1 => (left | right, if superchip { None } else { Some(0) }),
                    2 => (left & right, if superchip { None } else { Some(0) }),
                    3 => (left ^ right, if superchip { None } else { Some(0) }),
                    4 => {
                        let (result, carry) = left.overflowing_add(right);
                        (result, Some(carry as u8))
                    }
                    5 => (left.wrapping_sub(right), Some((left >= right) as u8)),
                    6 => (operand >> 1, Some(operand & 1)),
                    7 => (right.wrapping_sub(left), Some((right >= left) as u8)),
                    14 => (operand << 1, Some(operand >> 7)),
                    _ => return Err(Error::Opcode),
                };
                self.registers[first] = value;
                if let Some(flag) = flag {
                    self.registers[15] = flag;
                }
            }
            9 if nibble == 0 => {
                if left != right {
                    self.pc += 2;
                }
            }
            10 => self.index = target,
            11 => self.pc = target + self.registers[if superchip { first } else { 0 }] as u16,
            12 => {
                self.random ^= self.random << 13;
                self.random ^= self.random >> 17;
                self.random ^= self.random << 5;
                self.registers[first] = self.random as u8 & immediate;
            }
            13 => {
                let wide = nibble == 0 && superchip;
                let rows = if wide { 16 } else { nibble };
                let columns = if wide { 16 } else { 8 };
                self.range(rows * if wide { 2 } else { 1 })?;
                self.registers[15] = 0;
                for row in 0..rows {
                    let address = self.index as usize + row * if wide { 2 } else { 1 };
                    let sprite = (self.memory[address] as u16) << 8
                        | if wide {
                            self.memory[address + 1] as u16
                        } else {
                            0
                        };
                    for column in 0..columns {
                        if sprite & (0x8000 >> column) == 0 {
                            continue;
                        }
                        let x = left as usize % self.width + column;
                        let y = right as usize % self.height + row;
                        if superchip && (x >= self.width || y >= self.height) {
                            continue;
                        }
                        let pixel =
                            &mut self.pixels[(y % self.height) * self.width + x % self.width];
                        self.registers[15] |= *pixel;
                        *pixel ^= 1;
                    }
                }
                self.dirty = true;
            }
            14 => {
                let down = left < 16 && keys & (1u16 << left) != 0;
                match immediate {
                    0x9e => {
                        if down {
                            self.pc += 2;
                        }
                    }
                    0xa1 => {
                        if !down {
                            self.pc += 2;
                        }
                    }
                    _ => return Err(Error::Opcode),
                }
            }
            15 => match immediate {
                0x07 => self.registers[first] = self.delay,
                0x0a => {
                    if pressed == 0 {
                        self.pc -= 2;
                    } else {
                        self.registers[first] = pressed.trailing_zeros() as u8;
                    }
                }
                0x15 => self.delay = left,
                0x18 => self.sound = left,
                0x1e => self.index = self.index.wrapping_add(left as u16),
                0x29 => self.index = 0x50 + (left & 15) as u16 * 5,
                0x30 if superchip => self.index = 0xa0 + (left & 15) as u16 * 10,
                0x33 => {
                    let range = self.range(3)?;
                    self.memory[range].copy_from_slice(&[left / 100, left / 10 % 10, left % 10]);
                }
                0x55 => {
                    let range = self.range(first + 1)?;
                    self.memory[range].copy_from_slice(&self.registers[..=first]);
                    if !superchip {
                        self.index += first as u16 + 1;
                    }
                }
                0x65 => {
                    let range = self.range(first + 1)?;
                    self.registers[..=first].copy_from_slice(&self.memory[range]);
                    if !superchip {
                        self.index += first as u16 + 1;
                    }
                }
                0x75 if superchip && first < 8 => {
                    self.flags[..=first].copy_from_slice(&self.registers[..=first])
                }
                0x85 if superchip && first < 8 => {
                    self.registers[..=first].copy_from_slice(&self.flags[..=first])
                }
                _ => return Err(Error::Opcode),
            },
            _ => return Err(Error::Opcode),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_place_init_reuses_storage_and_clears_previous_game_state() {
        let mut storage = core::mem::MaybeUninit::uninit();
        let machine = Machine::init(&mut storage, &DEMO, Mode::SuperChip, 17).unwrap();
        machine.pixels.fill(1);
        machine.memory[4000] = 99;
        machine.registers.fill(23);
        machine.flags.fill(9);
        machine.stack_len = 16;
        machine.halted = true;
        let address = machine as *mut Machine as usize;
        let machine = Machine::init(&mut storage, &[0x60, 0x07], Mode::Chip8, 0).unwrap();
        assert_eq!(machine as *mut Machine as usize, address);
        assert!(machine.pixels.iter().all(|&pixel| pixel == 0));
        assert_eq!(machine.memory[4000], 0);
        assert_eq!(&machine.memory[0x50..0xa0], &FONT);
        assert_eq!(machine.flags, [0; 8]);
        assert_eq!(machine.stack_len, 0);
        assert_eq!((machine.width, machine.height, machine.random), (64, 32, 1));
        assert!(!machine.halted);
        machine.step(0, 0).unwrap();
        assert_eq!(machine.registers[0], 7);
        assert!(Machine::init(&mut storage, &[], Mode::Chip8, 1).is_err());
    }
    #[test]
    fn bundled_demo_runs_and_animates() {
        let mut machine = Machine::new(&DEMO, Mode::Chip8, 1).unwrap();
        let mut drew_pixels = false;
        for step in 0..1000 {
            machine.step(0, 0).unwrap();
            drew_pixels |= machine.pixels.iter().any(|&pixel| pixel == 1);
            if step % 16 == 0 {
                machine.tick();
            }
        }
        assert!(drew_pixels);
        assert!(machine.registers[0] > 0);
    }
    #[test]
    fn arithmetic_calls_and_bounds() {
        let mut machine = Machine::new(
            &[0x60, 255, 0x61, 1, 0x80, 0x14, 0xaf, 0xff, 0xf0, 0x33],
            Mode::Chip8,
            1,
        )
        .unwrap();
        for _ in 0..3 {
            machine.step(0, 0).unwrap();
        }
        assert_eq!((machine.registers[0], machine.registers[15]), (0, 1));
        machine.step(0, 0).unwrap();
        assert_eq!(machine.step(0, 0), Err(Error::Address));
        machine.pc = 4095;
        assert_eq!(machine.step(0, 0), Err(Error::Address));
        let mut recursive = Machine::new(&[0x22, 0], Mode::Chip8, 1).unwrap();
        for _ in 0..16 {
            recursive.step(0, 0).unwrap();
        }
        assert_eq!(recursive.step(0, 0), Err(Error::Stack));
    }
    #[test]
    fn superchip_resolution_scroll_exit_and_collision() {
        let mut machine = Machine::new(
            &[
                0, 0xff, 0xa0, 0x50, 0xd0, 0x15, 0xd0, 0x15, 0, 0xfb, 0, 0xfd,
            ],
            Mode::SuperChip,
            1,
        )
        .unwrap();
        for _ in 0..4 {
            machine.step(0, 0).unwrap();
        }
        assert_eq!(
            (machine.width, machine.height, machine.registers[15]),
            (128, 64, 1)
        );
        assert!(machine.pixels.iter().all(|&value| value == 0));
        machine.pixels[0] = 1;
        machine.step(0, 0).unwrap();
        assert_eq!(machine.pixels[4], 1);
        machine.step(0, 0).unwrap();
        assert!(machine.halted);
    }
}

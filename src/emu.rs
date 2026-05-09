use std::error::Error;
use std::fmt::Display;

const RAM_SIZE: u16 = 0x1000;
const STACK_SIZE: u8 = 16;

// Note that this is in pixels.
pub const DISPLAY_COLS: u16 = 64;
pub const DISPLAY_ROWS: u16 = 32;
pub const UNRESERVED_START: u16 = 0x200;
pub const DEFAULT_FONT_START: u16 = 0x0;
pub const DEFAULT_FONT_LEN: u16 = 0x5;

const FLAG_REG: usize = 0xf;

const DEFAULT_FONT: [[u8; 5]; 16] = [
    [0xf0, 0x90, 0x90, 0x90, 0xf0], // 0
    [0x20, 0x60, 0x20, 0x20, 0x70], // 1
    [0xf0, 0x10, 0xf0, 0x80, 0xf0], // 2
    [0xf0, 0x10, 0xf0, 0x10, 0xf0], // 3
    [0x90, 0x90, 0xf0, 0x10, 0x10], // 4
    [0xf0, 0x80, 0xf0, 0x10, 0xf0], // 5
    [0xf0, 0x80, 0xf0, 0x90, 0xf0], // 6
    [0xf0, 0x10, 0x20, 0x40, 0x40], // 7
    [0xf0, 0x90, 0xf0, 0x90, 0xf0], // 8
    [0xf0, 0x90, 0xf0, 0x10, 0xf0], // 9
    [0xf0, 0x90, 0xf0, 0x90, 0x90], // A
    [0xe0, 0x90, 0xe0, 0x90, 0xe0], // B
    [0xf0, 0x80, 0x80, 0x80, 0xf0], // C
    [0xe0, 0x90, 0x90, 0x90, 0xe0], // D
    [0xf0, 0x80, 0xf0, 0x80, 0xf0], // E
    [0xf0, 0x80, 0xf0, 0x80, 0x80], // F
];

#[derive(Debug, PartialEq, Eq)]
pub enum EmuError {
    // Invalid address
    AddressError(u16),
    // Unable to load data of size .1 at addr .0 - too large
    LoadError(u16, u16),
    // Invalid instruction .1 at .0
    InvalidInstruction(u16, u16),
    // Attempt to push more than STACK_SIZE values to stack at addr
    StackOverflowError(u16),
    // Attempt to pop from an empty stack at addr
    StackUnderflowError(u16),
}

// TODO: macros for pulling out vx, vy, nibble, and byte

impl Display for EmuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AddressError(addr) => {
                write!(f, "Attempt to access invalid address: 0x{:x}", addr)
            }
            Self::LoadError(addr, size) => {
                write!(
                    f,
                    "Unable to load 0x{:x} bytes of data at 0x{:x}",
                    size, addr
                )
            }
            Self::InvalidInstruction(addr, instr) => {
                write!(f, "Invalid instruction 0x{:x} at 0x{:x}", instr, addr)
            }
            Self::StackOverflowError(addr) => {
                write!(f, "Stack overflow error at 0x{:x}", addr)
            }
            Self::StackUnderflowError(addr) => {
                write!(f, "Attempt to pop from empty stack at 0x{:x}", addr)
            }
        }
    }
}

impl Error for EmuError {}

#[derive(Default)]
struct Registers {
    pc: u16,
    vx: [u8; 16],
    i: u16,
    dt: u8,
    st: u8,
}

pub struct Emu {
    regs: Registers,
    ram: Vec<u8>,
    stack: Vec<u16>,

    frame_buff: Vec<u8>,
}

impl Default for Emu {
    fn default() -> Emu {
        Self::new()
    }
}

// TODO: I should really do ram/read writes through a method that checks
// the address so that I don't have to remember to check the address in every
// call. Maybe even add a struct with a non public member

impl Emu {
    pub fn new() -> Emu {
        let mut emu = Emu {
            regs: Registers::default(),
            ram: vec![0; RAM_SIZE as usize],
            stack: Vec::with_capacity(STACK_SIZE as usize),
            frame_buff: vec![0; (DISPLAY_COLS * DISPLAY_ROWS) as usize],
        };

        let mut addr = 0x0;
        for sprite in DEFAULT_FONT {
            let len = sprite.len();
            emu.ram[addr..addr + len].copy_from_slice(&sprite);
            addr += len;
        }

        emu
    }

    #[cfg(test)]
    pub fn reset(&mut self) {
        self.regs = Registers::default();
        self.ram.fill(0);
        self.stack.fill(0);
        self.frame_buff.fill(0);
    }

    pub fn load(&mut self, addr: u16, data: Vec<u8>) -> Result<(), EmuError> {
        if !is_valid_addr(addr) {
            return Err(EmuError::AddressError(addr));
        }

        if data.len() > (RAM_SIZE - addr) as usize {
            return Err(EmuError::LoadError(addr, data.len() as u16));
        }

        self.ram[(addr as usize)..(addr as usize) + data.len()].copy_from_slice(&data);

        Ok(())
    }

    pub fn jump(&mut self, addr: u16) -> Result<(), EmuError> {
        if !is_valid_addr(addr) {
            return Err(EmuError::AddressError(addr));
        }

        self.regs.pc = addr;

        Ok(())
    }

    pub fn step(&mut self) -> Result<(), EmuError> {
        let pc = self.regs.pc;
        if !is_valid_addr(pc) {
            // TODO: ideally would check pc + 1 too
            return Err(EmuError::AddressError(pc));
        }

        let opcode = u16::from_be_bytes([self.ram[pc as usize], self.ram[(pc + 1) as usize]]);
        // println!("0x{:x}: {:x}", pc, opcode);

        self.regs.pc += 2;
        match (opcode & 0xf000) >> 12 {
            0x0 => match opcode {
                0x00e0 => self.op_cls(),
                0x00ee => self.op_ret(),
                _ => Err(EmuError::InvalidInstruction(pc, opcode)),
            },
            0x1 => self.op_jump(opcode),
            0x2 => self.op_call(opcode),
            0x3 => self.op_skip_equal_immediate(opcode),
            0x4 => self.op_skip_not_equal_immediate(opcode),
            0x5 => self.op_skip_equal_reg(opcode),
            0x6 => self.op_load_immediate(opcode),
            0x7 => self.op_add_immediate(opcode),
            0x8 => match opcode & 0x000f {
                0x0 => self.op_load_reg(opcode),
                0x1 => self.op_or(opcode),
                0x2 => self.op_and(opcode),
                0x3 => self.op_xor(opcode),
                0x4 => self.op_add_reg(opcode),
                0x5 => self.op_sub_reg(opcode),
                0x6 => self.op_shift_right(opcode),
                0x7 => self.op_subn_reg(opcode),
                0xe => self.op_shift_left(opcode),
                _ => Err(EmuError::InvalidInstruction(pc, opcode)),
            },
            0x9 => self.op_skip_not_equal_reg(opcode),
            0xa => self.op_load_index(opcode),
            0xb => self.op_jump_plus(opcode),
            0xc => self.op_random(opcode),
            0xd => self.op_display(opcode),
            0xe => match opcode & 0x00ff {
                0x9e => self.op_skip_if_key(opcode),
                0xa1 => self.op_skip_if_not_key(opcode),
                _ => Err(EmuError::InvalidInstruction(pc, opcode)),
            },
            0xf => match opcode & 0x00ff {
                0x07 => self.op_load_delay(opcode),
                0x0a => self.op_wait_for_key(opcode),
                0x15 => self.op_set_delay(opcode),
                0x18 => self.op_set_sound(opcode),
                0x1e => self.op_add_index(opcode),
                0x29 => self.op_load_sprite(opcode),
                0x33 => self.op_store_bcd(opcode),
                0x55 => self.op_store_regs(opcode),
                0x65 => self.op_load_regs(opcode),
                _ => Err(EmuError::InvalidInstruction(pc, opcode)),
            },
            _ => Err(EmuError::InvalidInstruction(pc, opcode)),
        }
    }

    pub fn frame_buffer(&self) -> &[u8] {
        &self.frame_buff
    }

    #[inline]
    fn op_ret(&mut self) -> Result<(), EmuError> {
        self.regs.pc = self
            .stack
            .pop()
            .ok_or(EmuError::StackUnderflowError(self.regs.pc))?;
        Ok(())
    }

    #[inline]
    fn op_cls(&mut self) -> Result<(), EmuError> {
        self.frame_buff.fill(0);
        Ok(())
    }

    #[inline]
    fn op_jump(&mut self, opcode: u16) -> Result<(), EmuError> {
        let addr: u16 = opcode & 0x0fff;
        self.regs.pc = addr;
        Ok(())
    }

    #[inline]
    fn op_call(&mut self, opcode: u16) -> Result<(), EmuError> {
        if self.stack.len() >= STACK_SIZE as usize {
            return Err(EmuError::StackOverflowError(self.regs.pc));
        }

        let addr = opcode & 0x0fff;
        self.stack.push(self.regs.pc);
        self.regs.pc = addr;

        Ok(())
    }

    #[inline]
    fn op_skip_equal_immediate(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = (opcode & 0x0f00) >> 8;
        let val = opcode & 0x00ff;

        if self.regs.vx[vx as usize] == (val as u8) {
            self.regs.pc += 2;
        }

        Ok(())
    }

    #[inline]
    fn op_skip_not_equal_immediate(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = (opcode & 0x0f00) >> 8;
        let val = opcode & 0x00ff;

        if self.regs.vx[vx as usize] != (val as u8) {
            self.regs.pc += 2;
        }

        Ok(())
    }

    #[inline]
    fn op_skip_equal_reg(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = (opcode & 0x0f00) >> 8;
        let vy = (opcode & 0x00f0) >> 4;

        if self.regs.vx[vx as usize] == self.regs.vx[vy as usize] {
            self.regs.pc += 2;
        }

        Ok(())
    }

    #[inline]
    fn op_skip_not_equal_reg(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = (opcode & 0x0f00) >> 8;
        let vy = (opcode & 0x00f0) >> 4;

        if self.regs.vx[vx as usize] != self.regs.vx[vy as usize] {
            self.regs.pc += 2;
        }

        Ok(())
    }

    #[inline]
    fn op_load_immediate(&mut self, opcode: u16) -> Result<(), EmuError> {
        let reg = (opcode & 0x0f00) >> 8;
        let val = opcode & 0x00ff;
        self.regs.vx[reg as usize] = val as u8;
        Ok(())
    }

    #[inline]
    fn op_add_immediate(&mut self, opcode: u16) -> Result<(), EmuError> {
        let reg = (opcode & 0x0f00) >> 8;
        let val = opcode & 0x00ff;
        // NOTE: for some reason this instruction does *not* set Vf on overflow. It just
        // overflows silently
        self.regs.vx[reg as usize] = self.regs.vx[reg as usize].wrapping_add(val as u8);
        Ok(())
    }

    #[inline]
    fn op_add_reg(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = ((opcode & 0x0f00) >> 8) as usize;
        let vy = ((opcode & 0x00f0) >> 4) as usize;

        let (res, overflowed) = self.regs.vx[vx].overflowing_add(self.regs.vx[vy]);
        self.regs.vx[vx] = res;
        self.regs.vx[FLAG_REG] = if overflowed { 1 } else { 0 };

        Ok(())
    }

    #[inline]
    fn op_load_reg(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = ((opcode & 0x0f00) >> 8) as usize;
        let vy = ((opcode & 0x00f0) >> 4) as usize;

        self.regs.vx[vx] = self.regs.vx[vy];

        Ok(())
    }

    #[inline]
    fn op_or(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = ((opcode & 0x0f00) >> 8) as usize;
        let vy = ((opcode & 0x00f0) >> 4) as usize;

        self.regs.vx[vx] |= self.regs.vx[vy];

        Ok(())
    }

    #[inline]
    fn op_and(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = ((opcode & 0x0f00) >> 8) as usize;
        let vy = ((opcode & 0x00f0) >> 4) as usize;

        self.regs.vx[vx] &= self.regs.vx[vy];

        Ok(())
    }

    #[inline]
    fn op_xor(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = ((opcode & 0x0f00) >> 8) as usize;
        let vy = ((opcode & 0x00f0) >> 4) as usize;

        self.regs.vx[vx] ^= self.regs.vx[vy];

        Ok(())
    }

    #[inline]
    fn op_sub_reg(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = ((opcode & 0x0f00) >> 8) as usize;
        let vy = ((opcode & 0x00f0) >> 4) as usize;

        let (res, overflowed) = self.regs.vx[vx].overflowing_sub(self.regs.vx[vy]);
        self.regs.vx[vx] = res;
        self.regs.vx[FLAG_REG] = if overflowed { 0 } else { 1 };

        Ok(())
    }

    #[inline]
    fn op_subn_reg(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = ((opcode & 0x0f00) >> 8) as usize;
        let vy = ((opcode & 0x00f0) >> 4) as usize;

        let (res, overflowed) = self.regs.vx[vy].overflowing_sub(self.regs.vx[vx]);
        self.regs.vx[vx] = res;
        self.regs.vx[FLAG_REG] = if overflowed { 0 } else { 1 };

        Ok(())
    }

    #[inline]
    fn op_shift_right(&mut self, opcode: u16) -> Result<(), EmuError> {
        // NOTE: some emulators do vx = vx >> vy. Most modern emulators ignore vy and shift by 1,
        // so that's what we're doing here as well.
        let vx = ((opcode & 0x0f00) >> 8) as usize;

        self.regs.vx[FLAG_REG] = self.regs.vx[vx] & 1;
        self.regs.vx[vx] = self.regs.vx[vx] >> 1;

        Ok(())
    }

    #[inline]
    fn op_shift_left(&mut self, opcode: u16) -> Result<(), EmuError> {
        // NOTE: some emulators do vx = vx >> vy. Most modern emulators ignore vy and shift by 1,
        // so that's what we're doing here as well.
        let vx = ((opcode & 0x0f00) >> 8) as usize;

        self.regs.vx[FLAG_REG] = self.regs.vx[vx] & 1;
        self.regs.vx[vx] = self.regs.vx[vx] << 1;

        Ok(())
    }

    #[inline]
    fn op_load_index(&mut self, opcode: u16) -> Result<(), EmuError> {
        let val = opcode & 0x0fff;
        self.regs.i = val as u16;

        Ok(())
    }

    #[inline]
    fn op_jump_plus(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_random(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_display(&mut self, opcode: u16) -> Result<(), EmuError> {
        let x_pixel = self.regs.vx[((opcode & 0x0f00) >> 8) as usize];
        let y_pixel = self.regs.vx[((opcode & 0x00f0) >> 4) as usize];
        let num_bytes = (opcode & 0x000f) as usize;
        if !is_valid_addr(self.regs.i) {
            return Err(EmuError::AddressError(self.regs.i));
        }
        let sprite_addr = self.regs.i as usize;
        let sprite_bytes = &self.ram[sprite_addr..(sprite_addr + num_bytes)];

        // TODO: This is pretty dirty but it works. There is surely a faster way to do this with bit manipulation
        for (row_idx, row) in sprite_bytes.into_iter().enumerate() {
            let mut col = 0;
            for i in (0..8).rev() {
                let idx = ((x_pixel as usize) + (col % DISPLAY_COLS as usize))
                    + ((y_pixel as usize) + (row_idx % DISPLAY_ROWS as usize))
                        * DISPLAY_COLS as usize;

                let state = (row >> i) & 1;
                if state == 1 && self.frame_buff[idx] == 1 {
                    self.regs.vx[FLAG_REG] = 1;
                } else {
                    self.regs.vx[FLAG_REG] = 0;
                }

                self.frame_buff[idx] ^= state;
                col += 1;
            }
        }

        Ok(())
    }

    #[inline]
    fn op_skip_if_key(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_skip_if_not_key(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_load_delay(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_wait_for_key(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_set_delay(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_set_sound(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_add_index(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = ((opcode & 0x0f00) >> 8) as usize;

        self.regs.i = self.regs.i.wrapping_add(self.regs.vx[vx].into());

        Ok(())
    }

    #[inline]
    fn op_load_sprite(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = (opcode & 0x0f00) >> 8;
        let sprite = self.regs.vx[vx as usize] as u16;
        self.regs.i = DEFAULT_FONT_START + (sprite * DEFAULT_FONT_LEN);
        Ok(())
    }

    #[inline]
    fn op_store_bcd(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = (opcode & 0x0f00) >> 8;
        let val = self.regs.vx[vx as usize];
        let index = self.regs.i as usize;

        if !is_valid_addr(index as u16) {
            return Err(EmuError::AddressError(index as u16));
        }

        self.ram[index..index + 3].copy_from_slice(&[val / 100, (val / 10) % 10, val % 10]);

        Ok(())
    }

    #[inline]
    fn op_store_regs(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = ((opcode & 0x0f00) >> 8) as usize;
        let index = self.regs.i as usize;

        if !is_valid_addr(index as u16) {
            return Err(EmuError::AddressError(index as u16));
        }

        self.ram[index..=(index + vx)].copy_from_slice(&self.regs.vx[0..=vx]);

        Ok(())
    }

    #[inline]
    fn op_load_regs(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = ((opcode & 0x0f00) >> 8) as usize;
        let index = self.regs.i as usize;

        if !is_valid_addr(index as u16) {
            return Err(EmuError::AddressError(index as u16));
        }

        self.regs.vx[0..=vx].copy_from_slice(&self.ram[index..=(index + vx)]);

        Ok(())
    }
}

fn is_valid_addr(addr: u16) -> bool {
    addr < RAM_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_load_addr() {
        let mut emu = Emu::new();

        assert_eq!(
            emu.load(RAM_SIZE, Vec::new()).unwrap_err(),
            EmuError::AddressError(RAM_SIZE)
        );

        assert_eq!(
            emu.load(RAM_SIZE + 0x1000, Vec::new()).unwrap_err(),
            EmuError::AddressError(RAM_SIZE + 0x1000)
        );
    }

    #[test]
    fn test_invalid_load_size() {
        let mut emu = Emu::new();

        assert_eq!(
            emu.load(UNRESERVED_START, vec![0; (RAM_SIZE + 1) as usize])
                .unwrap_err(),
            EmuError::LoadError(UNRESERVED_START, RAM_SIZE + 1)
        );
    }

    #[test]
    fn test_op_cls() {
        let mut emu = Emu::new();
        emu.frame_buff.fill(1);

        emu.op_cls().unwrap();
        assert!(emu.frame_buff.iter().all(|p| *p == 0));
    }

    #[test]
    fn test_op_ret() {
        let mut emu = Emu::new();
        emu.stack.push(0xfff);

        emu.op_ret().unwrap();

        assert_eq!(emu.op_ret(), Err(EmuError::StackUnderflowError(0xfff)));
    }

    #[test]
    fn test_op_jump() {
        let mut emu = Emu::new();
        emu.op_jump(0x1fff).unwrap();
    }

    #[test]
    fn test_op_skip_equal_immediate() {
        let mut emu = Emu::new();

        // should skip
        emu.op_skip_equal_immediate(0x3000).unwrap();
        assert_eq!(emu.regs.pc, 2);

        emu.reset();

        // should not skip
        emu.op_skip_equal_immediate(0x3001).unwrap();
        assert_eq!(emu.regs.pc, 0);

        emu.reset();

        // testing masking - should not skip
        emu.op_skip_equal_immediate(0x3fff).unwrap();
        assert_eq!(emu.regs.pc, 0);

        emu.reset();

        // testing masking - should skip
        emu.regs.vx[0xf] = 0xff;
        emu.op_skip_equal_immediate(0x3fff).unwrap();
        assert_eq!(emu.regs.pc, 2);
    }

    #[test]
    fn test_op_skip_equal_reg() {
        let mut emu = Emu::new();

        // v0 == v1, should skip
        emu.op_skip_equal_reg(0x5010).unwrap();
        assert_eq!(emu.regs.pc, 2);

        emu.reset();

        // v0 != v1, should not skip
        emu.regs.vx[0] = 0x1;
        emu.op_skip_equal_reg(0x5010).unwrap();
        assert_eq!(emu.regs.pc, 0);
    }

    #[test]
    fn test_op_skip_not_equal_immediate() {
        let mut emu = Emu::new();

        // v0 == 0, so should not skip
        emu.op_skip_not_equal_immediate(0x4000).unwrap();
        assert_eq!(emu.regs.pc, 0);

        emu.reset();

        // v0 == 0, so should skip
        emu.op_skip_not_equal_immediate(0x4001).unwrap();
        assert_eq!(emu.regs.pc, 2);

        emu.reset();

        // testing masking
        emu.op_skip_not_equal_immediate(0x4fff).unwrap();
        assert_eq!(emu.regs.pc, 2);

        emu.reset();

        // testing masking
        emu.regs.vx[0xf] = 0xff;
        emu.op_skip_not_equal_immediate(0x4fff).unwrap();
        assert_eq!(emu.regs.pc, 0);
    }

    #[test]
    fn test_op_skip_not_equal_reg() {
        let mut emu = Emu::new();

        // v0 == v1, should not skip
        emu.op_skip_not_equal_reg(0x9010).unwrap();
        assert_eq!(emu.regs.pc, 0);

        emu.reset();

        // v0 != v1, should skip
        emu.regs.vx[0] = 0x1;
        emu.op_skip_not_equal_reg(0x9010).unwrap();
        assert_eq!(emu.regs.pc, 2);
    }

    #[test]
    fn test_op_load_immediate() {
        let mut emu = Emu::new();
        emu.op_load_immediate(0x60ff).unwrap();
        assert_eq!(emu.regs.vx[0], 0xff);

        emu.op_load_immediate(0x6e0f).unwrap();
        assert_eq!(emu.regs.vx[0xe], 0x0f);
    }

    #[test]
    fn test_op_add_immediate() {
        let mut emu = Emu::new();

        emu.op_add_immediate(0x7005).unwrap();
        assert_eq!(emu.regs.vx[0], 0x05);

        emu.op_add_immediate(0x7005).unwrap();
        assert_eq!(emu.regs.vx[0], 0xa);

        // Test overflow
        emu.regs.vx[0xe] = 0x3;
        emu.op_add_immediate(0x7eff).unwrap();
        assert_eq!(emu.regs.vx[0xe], 0x2);
    }

    #[test]
    fn test_op_add_reg() {
        let mut emu = Emu::new();

        emu.regs.vx[0] = 1;
        emu.regs.vx[1] = 2;
        emu.op_add_reg(0x8014).unwrap();
        assert_eq!(emu.regs.vx[0], 3);
        assert_eq!(emu.regs.vx[FLAG_REG], 0);

        emu.regs.vx[5] = 4;
        emu.regs.vx[6] = 4;
        emu.op_add_reg(0x8564).unwrap();
        assert_eq!(emu.regs.vx[5], 8);
        assert_eq!(emu.regs.vx[FLAG_REG], 0);

        // Test overflow
        emu.regs.vx[5] = 3;
        emu.regs.vx[6] = 0xff;
        emu.op_add_reg(0x8564).unwrap();
        assert_eq!(emu.regs.vx[5], 2);
        assert_eq!(emu.regs.vx[FLAG_REG], 1);
    }

    #[test]
    fn test_op_load_reg() {
        let mut emu = Emu::new();
        emu.regs.vx[7] = 1;
        emu.regs.vx[8] = 2;
        emu.op_load_reg(0x8780).unwrap();
        assert_eq!(emu.regs.vx[7], 2);
    }

    #[test]
    fn test_op_or() {
        let mut emu = Emu::new();

        emu.regs.vx[0xa] = 0xf0;
        emu.regs.vx[0xb] = 0x03;
        emu.op_or(0x8ab1).unwrap();
        assert_eq!(emu.regs.vx[0xa], 0xf3);
    }

    #[test]
    fn test_op_and() {
        let mut emu = Emu::new();

        emu.regs.vx[0xa] = 0xf4;
        emu.regs.vx[0xb] = 0xf3;
        emu.op_and(0x8ab2).unwrap();
        assert_eq!(emu.regs.vx[0xa], 0xf0);
    }

    #[test]
    fn test_op_xor() {
        let mut emu = Emu::new();

        emu.regs.vx[0xa] = 0x54;
        emu.regs.vx[0xb] = 0x53;
        emu.op_xor(0x8ab3).unwrap();
        assert_eq!(emu.regs.vx[0xa], 0x7);
    }

    #[test]
    fn test_op_load_index() {
        let mut emu = Emu::new();
        emu.op_load_index(0xafff).unwrap();
        assert_eq!(emu.regs.i, 0xfff);

        emu.op_load_index(0xa000).unwrap();
        assert_eq!(emu.regs.i, 0x000);
    }

    #[test]
    fn test_op_sub_reg() {
        let mut emu = Emu::new();

        emu.regs.vx[0xc] = 10;
        emu.regs.vx[0xd] = 7;
        emu.op_sub_reg(0x8cd5).unwrap();
        assert_eq!(emu.regs.vx[0xc], 3);
        assert_eq!(emu.regs.vx[FLAG_REG], 1);

        emu.regs.vx[0xc] = 3;
        emu.regs.vx[0xd] = 9;
        emu.op_sub_reg(0x8cd5).unwrap();
        assert_eq!(emu.regs.vx[0xc], 250);
        assert_eq!(emu.regs.vx[FLAG_REG], 0);
    }

    #[test]
    fn test_op_subn_reg() {
        let mut emu = Emu::new();

        emu.regs.vx[0xc] = 7;
        emu.regs.vx[0xd] = 10;
        emu.op_subn_reg(0x8cd7).unwrap();
        assert_eq!(emu.regs.vx[0xc], 3);
        assert_eq!(emu.regs.vx[FLAG_REG], 1);

        emu.regs.vx[0xc] = 9;
        emu.regs.vx[0xd] = 3;
        emu.op_subn_reg(0x8cd7).unwrap();
        assert_eq!(emu.regs.vx[0xc], 250);
        assert_eq!(emu.regs.vx[FLAG_REG], 0);
    }

    #[test]
    fn test_op_shift_right() {
        let mut emu = Emu::new();

        emu.regs.vx[3] = 0x4;
        emu.op_shift_right(0x8306).unwrap();
        assert_eq!(emu.regs.vx[3], 2);
        assert_eq!(emu.regs.vx[FLAG_REG], 0);

        emu.regs.vx[3] = 0x3;
        emu.op_shift_right(0x8306).unwrap();
        assert_eq!(emu.regs.vx[3], 1);
        assert_eq!(emu.regs.vx[FLAG_REG], 1);
    }

    #[test]
    fn test_op_shift_left() {
        let mut emu = Emu::new();

        emu.regs.vx[3] = 0x4;
        emu.op_shift_left(0x8306).unwrap();
        assert_eq!(emu.regs.vx[3], 8);
        assert_eq!(emu.regs.vx[FLAG_REG], 0);

        emu.regs.vx[3] = 0x3;
        emu.op_shift_left(0x8306).unwrap();
        assert_eq!(emu.regs.vx[3], 6);
        assert_eq!(emu.regs.vx[FLAG_REG], 1);
    }

    // #[test]
    // fn test_op_display() {
    //     let mut emu = Emu::new();
    //     // TODO:
    // }

    #[test]
    fn test_op_load_sprite() {
        let mut emu = Emu::new();
        for i in 0..16 {
            emu.regs.vx[0] = i as u8;
            emu.op_load_sprite(0xf029).unwrap();
            assert_eq!(emu.regs.i, DEFAULT_FONT_START + i * DEFAULT_FONT_LEN);
            assert_eq!(
                &emu.ram[(emu.regs.i as usize)..(emu.regs.i + DEFAULT_FONT_LEN) as usize],
                &DEFAULT_FONT[i as usize]
            );
        }
    }

    #[test]
    fn test_op_call() {
        let mut emu = Emu::new();
        emu.jump(0x200).unwrap();
        emu.op_call(0x2fff).unwrap();
        assert_eq!(emu.stack.pop(), Some(0x200));
    }

    #[test]
    fn test_op_store_regs() {
        let mut emu = Emu::new();
        emu.regs.vx[0..16]
            .copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        emu.regs.i = 0;
        emu.op_store_regs(0xff55).unwrap();
        for i in 0..16 {
            assert_eq!(emu.ram[i as usize], i + 1);
        }

        emu.reset();

        emu.regs.vx[0..16]
            .copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        emu.regs.i = 3;
        emu.op_store_regs(0xf455).unwrap();

        let index = emu.regs.i as usize;
        assert_eq!(emu.ram[index..=(index + 4)], [1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_op_load_regs() {
        let mut emu = Emu::new();
        emu.ram[0..16].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        emu.regs.i = 0;
        emu.op_load_regs(0xff65).unwrap();
        for i in 0..16 {
            assert_eq!(emu.regs.vx[i as usize], i + 1);
        }

        emu.reset();

        emu.ram[0..16].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        emu.regs.i = 3;
        emu.op_load_regs(0xf465).unwrap();
        for i in 0..4 {
            assert_eq!(emu.regs.vx[i as usize], i + 4);
        }
    }

    #[test]
    fn test_add_index() {
        let mut emu = Emu::new();

        emu.regs.vx[0] = 1;
        emu.op_add_index(0xf01e).unwrap();
        assert_eq!(emu.regs.i, 1);

        emu.regs.vx[5] = 4;
        emu.op_add_index(0xf51e).unwrap();
        assert_eq!(emu.regs.i, 5);

        // Test overflow
        emu.regs.i = 0xfffe;
        emu.regs.vx[6] = 0x3;
        emu.op_add_index(0xf61e).unwrap();
        assert_eq!(emu.regs.i, 1);
    }

    #[test]
    fn test_op_store_bcd() {
        let mut emu = Emu::new();
        emu.regs.vx[0] = 123;
        emu.op_store_bcd(0xf033).unwrap();
        assert_eq!(emu.ram[..3], [1, 2, 3]);

        emu.regs.vx[0] = 255;
        emu.op_store_bcd(0xf033).unwrap();
        assert_eq!(emu.ram[..3], [2, 5, 5]);

        emu.regs.vx[0] = 000;
        emu.op_store_bcd(0xf033).unwrap();
        assert_eq!(emu.ram[..3], [0, 0, 0]);

        emu.regs.vx[7] = 205;
        emu.regs.i = 0x10;
        emu.op_store_bcd(0xf733).unwrap();
        assert_eq!(emu.ram[0x10..0x13], [2, 0, 5]);
    }
}

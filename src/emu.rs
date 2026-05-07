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

#[cfg(test)]
pub const RESERVED_END: u16 = 0x1ff;

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
    sp: u8,
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
                0x2 => self.op_xor(opcode),
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
    fn incr_pc(&mut self) {
        self.regs.pc += 2;
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
        self.incr_pc();
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
        self.stack.push(self.regs.pc + 2);
        self.regs.pc = addr;

        Ok(())
    }

    #[inline]
    fn op_skip_equal_immediate(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_skip_not_equal_immediate(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = (opcode & 0x0f00) >> 8;
        let val = opcode & 0x00ff;

        if self.regs.vx[vx as usize] != (val as u8) {
            self.regs.pc += 4;
        } else {
            self.incr_pc();
        }
        Ok(())
    }

    #[inline]
    fn op_skip_equal_reg(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_skip_not_equal_reg(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_load_immediate(&mut self, opcode: u16) -> Result<(), EmuError> {
        let reg = (opcode & 0x0f00) >> 8;
        let val = opcode & 0x00ff;
        self.regs.vx[reg as usize] = val as u8;
        self.incr_pc();
        Ok(())
    }

    #[inline]
    fn op_add_immediate(&mut self, opcode: u16) -> Result<(), EmuError> {
        let reg = (opcode & 0x0f00) >> 8;
        let val = opcode & 0x00ff;
        // NOTE: for some reason this instruction does *not* set Vf on overflow. It just
        // overflows silently
        self.regs.vx[reg as usize] = self.regs.vx[reg as usize].wrapping_add(val as u8);
        self.incr_pc();
        Ok(())
    }

    #[inline]
    fn op_add_reg(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_load_reg(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_or(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_xor(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_sub_reg(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_shift_right(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_subn_reg(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_shift_left(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_load_index(&mut self, opcode: u16) -> Result<(), EmuError> {
        let val = opcode & 0x0fff;
        self.regs.i = val as u16;
        self.incr_pc();

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
        // TODO: need to check this address
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
                    self.regs.vx[0xf] = 1;
                } else {
                    self.regs.vx[0xf] = 0;
                }

                self.frame_buff[idx] ^= state;
                col += 1;
            }
        }

        self.incr_pc();

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
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_load_sprite(&mut self, opcode: u16) -> Result<(), EmuError> {
        let vx = (opcode & 0x0f00) >> 8;
        let sprite = self.regs.vx[vx as usize] as u16;
        self.regs.i = DEFAULT_FONT_START + (sprite * DEFAULT_FONT_LEN);
        self.incr_pc();
        Ok(())
    }

    #[inline]
    fn op_store_bcd(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_store_regs(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_load_regs(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }
}

fn is_valid_addr(addr: u16) -> bool {
    if (addr > 0x1ff) && (addr < RAM_SIZE) {
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_load_addr() {
        let mut emu = Emu::new();

        assert_eq!(
            emu.load(0x0000, Vec::new()).unwrap_err(),
            EmuError::AddressError(0x0)
        );

        assert_eq!(
            emu.load(RESERVED_END, Vec::new()).unwrap_err(),
            EmuError::AddressError(RESERVED_END)
        );

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
        assert_eq!(emu.regs.pc, 2);
    }

    #[test]
    fn test_op_ret() {
        let mut emu = Emu::new();
        emu.stack.push(0xfff);

        emu.op_ret().unwrap();
        assert_eq!(emu.regs.pc, 0xfff);

        assert_eq!(emu.op_ret(), Err(EmuError::StackUnderflowError(0xfff)));
    }

    #[test]
    fn test_op_jump() {
        let mut emu = Emu::new();
        emu.op_jump(0x1fff).unwrap();
        assert_eq!(emu.regs.pc, 0xfff);
    }

    #[test]
    fn test_op_skip_not_equal_immediate() {
        let mut emu = Emu::new();

        // v0 == 0, so should not skip
        emu.op_skip_not_equal_immediate(0xf000).unwrap();
        assert_eq!(emu.regs.pc, 2);

        emu.reset();

        // v0 == 0, so should skip
        emu.op_skip_not_equal_immediate(0xf001).unwrap();
        assert_eq!(emu.regs.pc, 4);

        emu.reset();

        // testing masking
        emu.op_skip_not_equal_immediate(0xffff).unwrap();
        assert_eq!(emu.regs.pc, 4);

        emu.reset();

        // testing masking
        emu.regs.vx[0xf] = 0xff;
        emu.op_skip_not_equal_immediate(0xffff).unwrap();
        assert_eq!(emu.regs.pc, 2);
    }

    #[test]
    fn test_op_load_immediate() {
        let mut emu = Emu::new();
        emu.op_load_immediate(0x60ff).unwrap();
        assert_eq!(emu.regs.vx[0], 0xff);
        assert_eq!(emu.regs.pc, 2);

        emu.op_load_immediate(0x6e0f).unwrap();
        assert_eq!(emu.regs.vx[0xe], 0x0f);
        assert_eq!(emu.regs.pc, 4);
    }

    #[test]
    fn test_op_add_immediate() {
        let mut emu = Emu::new();

        emu.op_add_immediate(0x7005).unwrap();
        assert_eq!(emu.regs.vx[0], 0x05);
        assert_eq!(emu.regs.pc, 2);

        emu.op_add_immediate(0x7005).unwrap();
        assert_eq!(emu.regs.vx[0], 0xa);
        assert_eq!(emu.regs.pc, 4);

        // Test overflow
        emu.regs.vx[0xe] = 0x3;
        emu.op_add_immediate(0x7eff).unwrap();
        assert_eq!(emu.regs.vx[0xe], 0x2);
        assert_eq!(emu.regs.pc, 6);
    }

    #[test]
    fn test_op_load_index() {
        let mut emu = Emu::new();
        emu.op_load_index(0xafff).unwrap();
        assert_eq!(emu.regs.i, 0xfff);
        assert_eq!(emu.regs.pc, 2);

        emu.op_load_index(0xa000).unwrap();
        assert_eq!(emu.regs.i, 0x000);
        assert_eq!(emu.regs.pc, 4);
    }

    // #[test]
    // fn test_op_display() {
    //     let mut emu = Emu::new();
    //     // TODO:
    //     assert_eq!(emu.regs.pc, 2);
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
            assert_eq!(emu.regs.pc, (i * 2) + 2);
        }
    }

    #[test]
    fn test_op_call() {
        let mut emu = Emu::new();
        emu.jump(0x200).unwrap();
        emu.op_call(0x2fff).unwrap();
        assert_eq!(emu.regs.pc, 0xfff);
        assert_eq!(emu.stack.pop(), Some(0x202));
    }
}

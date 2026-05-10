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

fn is_valid_addr(addr: u16) -> bool {
    addr < RAM_SIZE
}

fn check_addr_range(addr: u16, num_bytes: u16) -> Result<(), EmuError> {
    if !is_valid_addr(addr) {
        return Err(EmuError::AddressError(addr));
    }

    if !is_valid_addr(addr + num_bytes) {
        // This is the first invalid address of the range
        return Err(EmuError::AddressError(RAM_SIZE));
    }

    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub enum EmuError {
    AddressError(u16),
    InvalidInstruction(u16),
    // Attempt to push more than STACK_SIZE values to stack
    StackOverflowError,
    // Attempt to pop from an empty stack
    StackUnderflowError,
}

impl Display for EmuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AddressError(addr) => {
                write!(f, "Attempt to access invalid address: 0x{:x}", addr)
            }
            Self::InvalidInstruction(instr) => {
                write!(f, "Invalid instruction 0x{:x}", instr)
            }
            Self::StackOverflowError => {
                write!(f, "Stack overflow error")
            }
            Self::StackUnderflowError => {
                write!(f, "Attempt to pop from empty stack")
            }
        }
    }
}

impl Error for EmuError {}

#[derive(Debug)]
pub struct EmuErrorWithCtx {
    error: EmuError,
    ctx: Registers,
}

impl Error for EmuErrorWithCtx {}

impl Display for EmuErrorWithCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Err: {}, Context: {:?}", self.error, self.ctx)
    }
}

#[derive(Default, Clone, Debug)]
struct Registers {
    pc: u16,
    vx: [u8; 16],
    i: u16,
    dt: u8,
    st: u8,
}

struct Ram {
    ram: Vec<u8>,
}

impl Ram {
    fn default() -> Ram {
        Ram {
            ram: vec![0; RAM_SIZE as usize],
        }
    }

    fn read(&self, addr: u16) -> Result<u8, EmuError> {
        self.ram
            .get(addr as usize)
            .copied()
            .ok_or(EmuError::AddressError(addr))
    }

    fn read_u16(&self, addr: u16) -> Result<u16, EmuError> {
        let bytes = [self.read(addr)?, self.read(addr + 1)?];
        Ok(u16::from_be_bytes(bytes))
    }

    fn read_slice(&self, addr: u16, num_bytes: u16) -> Result<&[u8], EmuError> {
        check_addr_range(addr, num_bytes)?;

        Ok(&self.ram[(addr as usize)..(addr + num_bytes) as usize])
    }

    fn write_slice(&mut self, addr: u16, data: &[u8]) -> Result<(), EmuError> {
        // TODO: is there a better way to handle this length conversion?
        check_addr_range(addr, data.len() as u16)?;

        self.ram[(addr as usize)..((addr as usize) + data.len())].copy_from_slice(data);
        Ok(())
    }

    #[cfg(test)]
    fn clear(&mut self) {
        self.ram.fill(0);
    }
}

macro_rules! op {
    ($opcode:expr) => {
        ($opcode & 0xf000u16) >> 12
    };
}

macro_rules! vx {
    ($opcode:expr) => {
        (($opcode & 0x0f00u16) >> 8) as u8
    };
}

macro_rules! vy {
    ($opcode:expr) => {
        (($opcode & 0x00f0u16) >> 4) as u8
    };
}

macro_rules! nibble {
    ($opcode:expr) => {
        ($opcode & 0x000fu16) as u8
    };
}

macro_rules! kk {
    ($opcode:expr) => {
        ($opcode & 0x00ffu16) as u8
    };
}

macro_rules! addr {
    ($opcode:expr) => {
        ($opcode & 0x0fffu16) as u16
    };
}

pub struct Emu {
    regs: Registers,
    ram: Ram,
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
            ram: Ram::default(),
            stack: Vec::with_capacity(STACK_SIZE as usize),
            frame_buff: vec![0; (DISPLAY_COLS * DISPLAY_ROWS) as usize],
        };

        let mut addr = 0x0;
        for sprite in DEFAULT_FONT {
            let len = sprite.len();
            emu.ram
                .write_slice(addr as u16, &sprite)
                .expect("Failed to load default font in to memory.");
            addr += len;
        }

        emu
    }

    #[cfg(test)]
    pub fn reset(&mut self) {
        self.regs = Registers::default();
        self.ram.clear();
        self.stack.fill(0);
        self.frame_buff.fill(0);
    }

    pub fn load(&mut self, addr: u16, data: Vec<u8>) -> Result<(), EmuError> {
        check_addr_range(addr, data.len() as u16)?;
        self.ram.write_slice(addr, &data)
    }

    pub fn jump(&mut self, addr: u16) -> Result<(), EmuError> {
        if !is_valid_addr(addr) {
            return Err(EmuError::AddressError(addr));
        }

        self.regs.pc = addr;

        Ok(())
    }

    pub fn step(&mut self) -> Result<(), EmuErrorWithCtx> {
        let pc = self.regs.pc;
        let opcode = self.ram.read_u16(pc).map_err(|e| EmuErrorWithCtx {
            error: e,
            ctx: self.regs.clone(),
        })?;

        // println!("0x{:x}: {:x}", pc, opcode);

        self.regs.pc += 2;
        let res = match op!(opcode) {
            0x0 => match opcode {
                0x00e0 => self.op_cls(),
                0x00ee => self.op_ret(),
                _ => Err(EmuError::InvalidInstruction(opcode)),
            },
            0x1 => self.op_jump(addr!(opcode)),
            0x2 => self.op_call(addr!(opcode)),
            0x3 => self.op_skip_equal_immediate(vx!(opcode), kk!(opcode)),
            0x4 => self.op_skip_not_equal_immediate(vx!(opcode), kk!(opcode)),
            0x5 => self.op_skip_equal_reg(vx!(opcode), vy!(opcode)),
            0x6 => self.op_load_immediate(vx!(opcode), kk!(opcode)),
            0x7 => self.op_add_immediate(vx!(opcode), kk!(opcode)),
            0x8 => match opcode & 0x000f {
                0x0 => self.op_load_reg(vx!(opcode), vy!(opcode)),
                0x1 => self.op_or(vx!(opcode), vy!(opcode)),
                0x2 => self.op_and(vx!(opcode), vy!(opcode)),
                0x3 => self.op_xor(vx!(opcode), vy!(opcode)),
                0x4 => self.op_add_reg(vx!(opcode), vy!(opcode)),
                0x5 => self.op_sub_reg(vx!(opcode), vy!(opcode)),
                0x6 => self.op_shift_right(vx!(opcode), vy!(opcode)),
                0x7 => self.op_subn_reg(vx!(opcode), vy!(opcode)),
                0xe => self.op_shift_left(vx!(opcode), vy!(opcode)),
                _ => Err(EmuError::InvalidInstruction(opcode)),
            },
            0x9 => self.op_skip_not_equal_reg(vx!(opcode), vy!(opcode)),
            0xa => self.op_load_index(addr!(opcode)),
            0xb => self.op_jump_plus(addr!(opcode)),
            0xc => self.op_random(vx!(opcode), kk!(opcode)),
            0xd => self.op_display(vx!(opcode), vy!(opcode), nibble!(opcode)),
            0xe => match opcode & 0x00ff {
                0x9e => self.op_skip_if_key(vx!(opcode)),
                0xa1 => self.op_skip_if_not_key(vx!(opcode)),
                _ => Err(EmuError::InvalidInstruction(opcode)),
            },
            0xf => match opcode & 0x00ff {
                0x07 => self.op_load_delay(vx!(opcode)),
                0x0a => self.op_wait_for_key(vx!(opcode)),
                0x15 => self.op_set_delay(vx!(opcode)),
                0x18 => self.op_set_sound(vx!(opcode)),
                0x1e => self.op_add_index(vx!(opcode)),
                0x29 => self.op_load_sprite(vx!(opcode)),
                0x33 => self.op_store_bcd(vx!(opcode)),
                0x55 => self.op_store_regs(vx!(opcode)),
                0x65 => self.op_load_regs(vx!(opcode)),
                _ => Err(EmuError::InvalidInstruction(opcode)),
            },
            _ => Err(EmuError::InvalidInstruction(opcode)),
        };

        if let Err(e) = res {
            let mut ctx = self.regs.clone();
            ctx.pc = pc;
            Err(EmuErrorWithCtx { error: e, ctx: ctx })
        } else {
            Ok(())
        }
    }

    pub fn frame_buffer(&self) -> &[u8] {
        &self.frame_buff
    }

    #[inline]
    fn op_ret(&mut self) -> Result<(), EmuError> {
        self.regs.pc = self.stack.pop().ok_or(EmuError::StackUnderflowError)?;
        Ok(())
    }

    #[inline]
    fn op_cls(&mut self) -> Result<(), EmuError> {
        self.frame_buff.fill(0);
        Ok(())
    }

    #[inline]
    fn op_jump(&mut self, addr: u16) -> Result<(), EmuError> {
        self.regs.pc = addr;
        Ok(())
    }

    #[inline]
    fn op_call(&mut self, addr: u16) -> Result<(), EmuError> {
        if self.stack.len() >= STACK_SIZE as usize {
            return Err(EmuError::StackOverflowError);
        }

        self.stack.push(self.regs.pc);
        self.regs.pc = addr;

        Ok(())
    }

    #[inline]
    fn op_skip_equal_immediate(&mut self, vx: u8, val: u8) -> Result<(), EmuError> {
        if self.regs.vx[vx as usize] == val {
            self.regs.pc += 2;
        }

        Ok(())
    }

    #[inline]
    fn op_skip_not_equal_immediate(&mut self, vx: u8, val: u8) -> Result<(), EmuError> {
        if self.regs.vx[vx as usize] != val as u8 {
            self.regs.pc += 2;
        }

        Ok(())
    }

    #[inline]
    fn op_skip_equal_reg(&mut self, vx: u8, vy: u8) -> Result<(), EmuError> {
        if self.regs.vx[vx as usize] == self.regs.vx[vy as usize] {
            self.regs.pc += 2;
        }

        Ok(())
    }

    #[inline]
    fn op_skip_not_equal_reg(&mut self, vx: u8, vy: u8) -> Result<(), EmuError> {
        if self.regs.vx[vx as usize] != self.regs.vx[vy as usize] {
            self.regs.pc += 2;
        }

        Ok(())
    }

    #[inline]
    fn op_load_immediate(&mut self, vx: u8, val: u8) -> Result<(), EmuError> {
        self.regs.vx[vx as usize] = val;
        Ok(())
    }

    #[inline]
    fn op_add_immediate(&mut self, vx: u8, val: u8) -> Result<(), EmuError> {
        // NOTE: for some reason this instruction does *not* set Vf on overflow. It just
        // overflows silently
        self.regs.vx[vx as usize] = self.regs.vx[vx as usize].wrapping_add(val);
        Ok(())
    }

    #[inline]
    fn op_add_reg(&mut self, vx: u8, vy: u8) -> Result<(), EmuError> {
        let (res, overflowed) =
            self.regs.vx[vx as usize].overflowing_add(self.regs.vx[vy as usize]);
        self.regs.vx[vx as usize] = res;
        self.regs.vx[FLAG_REG] = if overflowed { 1 } else { 0 };

        Ok(())
    }

    #[inline]
    fn op_load_reg(&mut self, vx: u8, vy: u8) -> Result<(), EmuError> {
        self.regs.vx[vx as usize] = self.regs.vx[vy as usize];

        Ok(())
    }

    #[inline]
    fn op_or(&mut self, vx: u8, vy: u8) -> Result<(), EmuError> {
        self.regs.vx[vx as usize] |= self.regs.vx[vy as usize];

        Ok(())
    }

    #[inline]
    fn op_and(&mut self, vx: u8, vy: u8) -> Result<(), EmuError> {
        self.regs.vx[vx as usize] &= self.regs.vx[vy as usize];

        Ok(())
    }

    #[inline]
    fn op_xor(&mut self, vx: u8, vy: u8) -> Result<(), EmuError> {
        self.regs.vx[vx as usize] ^= self.regs.vx[vy as usize];

        Ok(())
    }

    #[inline]
    fn op_sub_reg(&mut self, vx: u8, vy: u8) -> Result<(), EmuError> {
        let (res, overflowed) =
            self.regs.vx[vx as usize].overflowing_sub(self.regs.vx[vy as usize]);
        self.regs.vx[vx as usize] = res;
        self.regs.vx[FLAG_REG] = if overflowed { 0 } else { 1 };

        Ok(())
    }

    #[inline]
    fn op_subn_reg(&mut self, vx: u8, vy: u8) -> Result<(), EmuError> {
        let (res, overflowed) =
            self.regs.vx[vy as usize].overflowing_sub(self.regs.vx[vx as usize]);
        self.regs.vx[vx as usize] = res;
        self.regs.vx[FLAG_REG] = if overflowed { 0 } else { 1 };

        Ok(())
    }

    #[inline]
    fn op_shift_right(&mut self, vx: u8, vy: u8) -> Result<(), EmuError> {
        // NOTE: some emulators do vx = vx >> 1. I'm using vx = vy >> 1 because that's what octo
        // does
        let flag = self.regs.vx[vy as usize] & 1;
        self.regs.vx[vx as usize] = self.regs.vx[vy as usize] >> 1;

        // Important that we're setting vF after the above shift - this allows you to use vF as vY
        self.regs.vx[FLAG_REG] = flag;

        Ok(())
    }

    #[inline]
    fn op_shift_left(&mut self, vx: u8, vy: u8) -> Result<(), EmuError> {
        // NOTE: some emulators do vx = vx << 1. I'm using vx = vy << 1 because that's what octo
        // does
        let flag = (self.regs.vx[vy as usize] & 0x80) >> 7;
        self.regs.vx[vx as usize] = self.regs.vx[vy as usize] << 1;

        // Important that we're setting vF after the above shift - this allows you to use vF as vY
        self.regs.vx[FLAG_REG] = flag;

        Ok(())
    }

    #[inline]
    fn op_load_index(&mut self, addr: u16) -> Result<(), EmuError> {
        self.regs.i = addr;

        Ok(())
    }

    #[inline]
    fn op_jump_plus(&mut self, addr: u16) -> Result<(), EmuError> {
        self.regs.pc = addr + self.regs.vx[0] as u16;
        Ok(())
    }

    #[inline]
    fn op_random(&mut self, vx: u8, val: u8) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_display(&mut self, vx: u8, vy: u8, num_bytes: u8) -> Result<(), EmuError> {
        let x_pixel = self.regs.vx[vx as usize];
        let y_pixel = self.regs.vx[vy as usize];
        let sprite_bytes = self.ram.read_slice(self.regs.i, num_bytes as u16)?;

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
    fn op_skip_if_key(&mut self, vx: u8) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_skip_if_not_key(&mut self, vx: u8) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_load_delay(&mut self, vx: u8) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_wait_for_key(&mut self, vx: u8) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_set_delay(&mut self, vx: u8) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_set_sound(&mut self, vx: u8) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_add_index(&mut self, vx: u8) -> Result<(), EmuError> {
        self.regs.i = self.regs.i.wrapping_add(self.regs.vx[vx as usize].into());

        Ok(())
    }

    #[inline]
    fn op_load_sprite(&mut self, vx: u8) -> Result<(), EmuError> {
        let sprite = self.regs.vx[vx as usize] as u16;
        self.regs.i = DEFAULT_FONT_START + (sprite * DEFAULT_FONT_LEN);
        Ok(())
    }

    #[inline]
    fn op_store_bcd(&mut self, vx: u8) -> Result<(), EmuError> {
        let val = self.regs.vx[vx as usize];
        self.ram
            .write_slice(self.regs.i, &[val / 100, (val / 10) % 10, val % 10])
    }

    #[inline]
    fn op_store_regs(&mut self, vx: u8) -> Result<(), EmuError> {
        self.ram
            .write_slice(self.regs.i, &self.regs.vx[0..=(vx as usize)])
    }

    #[inline]
    fn op_load_regs(&mut self, vx: u8) -> Result<(), EmuError> {
        let vals = self.ram.read_slice(self.regs.i, (vx + 1) as u16)?;
        self.regs.vx[0..=(vx as usize)].copy_from_slice(vals);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ram_read() {
        let mut ram = Ram::default();

        assert_eq!(
            ram.read(RAM_SIZE + 5).unwrap_err(),
            EmuError::AddressError(RAM_SIZE + 5)
        );

        assert_eq!(
            ram.read(RAM_SIZE).unwrap_err(),
            EmuError::AddressError(RAM_SIZE)
        );

        ram.ram[(RAM_SIZE - 1) as usize] = 0xf;
        assert_eq!(ram.read(RAM_SIZE - 1).unwrap(), 0xf);

        ram.ram[0] = 0xa;
        assert_eq!(ram.read(0).unwrap(), 0xa);

        ram.ram[255] = 0xe;
        assert_eq!(ram.read(255).unwrap(), 0xe);
    }

    #[test]
    fn test_ram_read_u16() {
        let mut ram = Ram::default();
        ram.ram[0] = 0x83;
        ram.ram[1] = 0x45;
        assert_eq!(ram.read_u16(0x0).unwrap(), 0x8345);

        assert_eq!(
            ram.read_u16(RAM_SIZE - 1).unwrap_err(),
            EmuError::AddressError(RAM_SIZE)
        );
    }

    #[test]
    fn test_ram_read_slice() {
        let mut ram = Ram::default();
        ram.ram[0] = 0x83;
        ram.ram[1] = 0x45;
        ram.ram[2] = 0x56;
        ram.ram[3] = 0xff;
        ram.ram[4] = 0xfe;

        assert_eq!(ram.read_slice(0, 3).unwrap(), &[0x83, 0x45, 0x56]);
        assert_eq!(ram.read_slice(3, 2).unwrap(), &[0xff, 0xfe]);

        assert_eq!(
            ram.read_slice(RAM_SIZE + 5, 5).unwrap_err(),
            EmuError::AddressError(RAM_SIZE + 5)
        );

        assert_eq!(
            ram.read_slice(RAM_SIZE - 1, 5).unwrap_err(),
            EmuError::AddressError(RAM_SIZE)
        );
    }

    #[test]
    fn test_ram_write_slice() {
        let mut ram = Ram::default();

        ram.write_slice(0, &[1, 2, 3, 4, 5]).unwrap();
        assert_eq!(ram.ram[..5], [1, 2, 3, 4, 5]);

        ram.write_slice(0x10, &[1, 2, 3, 4, 5]).unwrap();
        assert_eq!(ram.ram[0x10..0x15], [1, 2, 3, 4, 5]);

        assert_eq!(
            ram.write_slice(RAM_SIZE, &[1, 2, 3]).unwrap_err(),
            EmuError::AddressError(RAM_SIZE)
        );

        assert_eq!(
            ram.write_slice(RAM_SIZE - 1, &[1, 2, 3]).unwrap_err(),
            EmuError::AddressError(RAM_SIZE)
        );
    }

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

        assert_eq!(emu.op_ret(), Err(EmuError::StackUnderflowError));
    }

    #[test]
    fn test_op_jump() {
        let mut emu = Emu::new();
        emu.op_jump(0xfff).unwrap();
        assert_eq!(emu.regs.pc, 0xfff);
    }

    #[test]
    fn test_op_skip_equal_immediate() {
        let mut emu = Emu::new();

        // should skip
        emu.op_skip_equal_immediate(0, 0).unwrap();
        assert_eq!(emu.regs.pc, 2);

        emu.reset();

        // should not skip
        emu.op_skip_equal_immediate(0, 1).unwrap();
        assert_eq!(emu.regs.pc, 0);

        emu.reset();

        // testing masking - should not skip
        emu.op_skip_equal_immediate(0xf, 0xff).unwrap();
        assert_eq!(emu.regs.pc, 0);

        emu.reset();

        // testing masking - should skip
        emu.regs.vx[0xf] = 0xff;
        emu.op_skip_equal_immediate(0xf, 0xff).unwrap();
        assert_eq!(emu.regs.pc, 2);
    }

    #[test]
    fn test_op_skip_equal_reg() {
        let mut emu = Emu::new();

        // v0 == v1, should skip
        emu.op_skip_equal_reg(0, 1).unwrap();
        assert_eq!(emu.regs.pc, 2);

        emu.reset();

        // v0 != v1, should not skip
        emu.regs.vx[0] = 0x1;
        emu.op_skip_equal_reg(0, 1).unwrap();
        assert_eq!(emu.regs.pc, 0);
    }

    #[test]
    fn test_op_skip_not_equal_immediate() {
        let mut emu = Emu::new();

        // v0 == 0, so should not skip
        emu.op_skip_not_equal_immediate(0, 0).unwrap();
        assert_eq!(emu.regs.pc, 0);

        emu.reset();

        // v0 == 0, so should skip
        emu.op_skip_not_equal_immediate(0, 1).unwrap();
        assert_eq!(emu.regs.pc, 2);

        emu.reset();

        // testing masking
        emu.op_skip_not_equal_immediate(0xf, 0xff).unwrap();
        assert_eq!(emu.regs.pc, 2);

        emu.reset();

        // testing masking
        emu.regs.vx[0xf] = 0xff;
        emu.op_skip_not_equal_immediate(0xf, 0xff).unwrap();
        assert_eq!(emu.regs.pc, 0);
    }

    #[test]
    fn test_op_skip_not_equal_reg() {
        let mut emu = Emu::new();

        // v0 == v1, should not skip
        emu.op_skip_not_equal_reg(0, 1).unwrap();
        assert_eq!(emu.regs.pc, 0);

        emu.reset();

        // v0 != v1, should skip
        emu.regs.vx[0] = 0x1;
        emu.op_skip_not_equal_reg(0, 1).unwrap();
        assert_eq!(emu.regs.pc, 2);
    }

    #[test]
    fn test_op_load_immediate() {
        let mut emu = Emu::new();
        emu.op_load_immediate(0, 0xff).unwrap();
        assert_eq!(emu.regs.vx[0], 0xff);

        emu.op_load_immediate(0xe, 0x0f).unwrap();
        assert_eq!(emu.regs.vx[0xe], 0x0f);
    }

    #[test]
    fn test_op_add_immediate() {
        let mut emu = Emu::new();

        emu.op_add_immediate(0, 5).unwrap();
        assert_eq!(emu.regs.vx[0], 5);

        emu.op_add_immediate(0, 5).unwrap();
        assert_eq!(emu.regs.vx[0], 10);

        // Test overflow
        emu.regs.vx[0xe] = 0x3;
        emu.op_add_immediate(0xe, 0xff).unwrap();
        assert_eq!(emu.regs.vx[0xe], 0x2);
    }

    #[test]
    fn test_op_add_reg() {
        let mut emu = Emu::new();

        emu.regs.vx[0] = 1;
        emu.regs.vx[1] = 2;
        emu.op_add_reg(0, 1).unwrap();
        assert_eq!(emu.regs.vx[0], 3);
        assert_eq!(emu.regs.vx[FLAG_REG], 0);

        emu.regs.vx[5] = 4;
        emu.regs.vx[6] = 4;
        emu.op_add_reg(5, 6).unwrap();
        assert_eq!(emu.regs.vx[5], 8);
        assert_eq!(emu.regs.vx[FLAG_REG], 0);

        // Test overflow
        emu.regs.vx[5] = 3;
        emu.regs.vx[6] = 0xff;
        emu.op_add_reg(5, 6).unwrap();
        assert_eq!(emu.regs.vx[5], 2);
        assert_eq!(emu.regs.vx[FLAG_REG], 1);
    }

    #[test]
    fn test_op_load_reg() {
        let mut emu = Emu::new();
        emu.regs.vx[7] = 1;
        emu.regs.vx[8] = 2;
        emu.op_load_reg(7, 8).unwrap();
        assert_eq!(emu.regs.vx[7], 2);
    }

    #[test]
    fn test_op_or() {
        let mut emu = Emu::new();

        emu.regs.vx[0xa] = 0xf0;
        emu.regs.vx[0xb] = 0x03;
        emu.op_or(0xa, 0xb).unwrap();
        assert_eq!(emu.regs.vx[0xa], 0xf3);
    }

    #[test]
    fn test_op_and() {
        let mut emu = Emu::new();

        emu.regs.vx[0xa] = 0xf4;
        emu.regs.vx[0xb] = 0xf3;
        emu.op_and(0xa, 0xb).unwrap();
        assert_eq!(emu.regs.vx[0xa], 0xf0);
    }

    #[test]
    fn test_op_xor() {
        let mut emu = Emu::new();

        emu.regs.vx[0xa] = 0x54;
        emu.regs.vx[0xb] = 0x53;
        emu.op_xor(0xa, 0xb).unwrap();
        assert_eq!(emu.regs.vx[0xa], 0x7);
    }

    #[test]
    fn test_op_load_index() {
        let mut emu = Emu::new();
        emu.op_load_index(0xfff).unwrap();
        assert_eq!(emu.regs.i, 0xfff);

        emu.op_load_index(0x000).unwrap();
        assert_eq!(emu.regs.i, 0x000);
    }

    #[test]
    fn test_op_sub_reg() {
        let mut emu = Emu::new();

        emu.regs.vx[0xc] = 10;
        emu.regs.vx[0xd] = 7;
        emu.op_sub_reg(0xc, 0xd).unwrap();
        assert_eq!(emu.regs.vx[0xc], 3);
        assert_eq!(emu.regs.vx[FLAG_REG], 1);

        emu.regs.vx[0xc] = 3;
        emu.regs.vx[0xd] = 9;
        emu.op_sub_reg(0xc, 0xd).unwrap();
        assert_eq!(emu.regs.vx[0xc], 250);
        assert_eq!(emu.regs.vx[FLAG_REG], 0);
    }

    #[test]
    fn test_op_subn_reg() {
        let mut emu = Emu::new();

        emu.regs.vx[0xc] = 7;
        emu.regs.vx[0xd] = 10;
        emu.op_subn_reg(0xc, 0xd).unwrap();
        assert_eq!(emu.regs.vx[0xc], 3);
        assert_eq!(emu.regs.vx[FLAG_REG], 1);

        emu.regs.vx[0xc] = 9;
        emu.regs.vx[0xd] = 3;
        emu.op_subn_reg(0xc, 0xd).unwrap();
        assert_eq!(emu.regs.vx[0xc], 250);
        assert_eq!(emu.regs.vx[FLAG_REG], 0);
    }

    #[test]
    fn test_op_shift_right() {
        let mut emu = Emu::new();

        // Basic shift - flag should not be set
        emu.regs.vx[4] = 0x4;
        emu.op_shift_right(3, 4).unwrap();
        assert_eq!(emu.regs.vx[3], 2);
        assert_eq!(emu.regs.vx[FLAG_REG], 0);

        // Basic shift - flag should be set
        emu.regs.vx[0] = 0x3;
        emu.op_shift_right(3, 0).unwrap();
        assert_eq!(emu.regs.vx[3], 1);
        assert_eq!(emu.regs.vx[FLAG_REG], 1);

        // Test shift self
        emu.regs.vx[3] = 0x3;
        emu.op_shift_right(3, 3).unwrap();
        assert_eq!(emu.regs.vx[3], 1);
        assert_eq!(emu.regs.vx[FLAG_REG], 1);

        // Test shift with flag reg (no carry)
        emu.regs.vx[0xf] = 0x4;
        emu.op_shift_right(3, 0xf).unwrap();
        assert_eq!(emu.regs.vx[3], 2);
        assert_eq!(emu.regs.vx[FLAG_REG], 0);

        // Test shift with flag reg (with carry)
        emu.regs.vx[0xf] = 0x3;
        emu.op_shift_right(3, 0xf).unwrap();
        assert_eq!(emu.regs.vx[3], 1);
        assert_eq!(emu.regs.vx[FLAG_REG], 1);
    }

    #[test]
    fn test_op_shift_left() {
        let mut emu = Emu::new();

        // Basic shift - flag should not be set
        emu.regs.vx[4] = 0x4;
        emu.op_shift_left(3, 4).unwrap();
        assert_eq!(emu.regs.vx[3], 8);
        assert_eq!(emu.regs.vx[FLAG_REG], 0);

        // Basic shift - flag should be set
        emu.regs.vx[4] = 0x81;
        emu.op_shift_left(3, 4).unwrap();
        assert_eq!(emu.regs.vx[3], 0x2);
        assert_eq!(emu.regs.vx[FLAG_REG], 1);

        // Test shift self
        emu.regs.vx[4] = 0x4;
        emu.op_shift_left(4, 4).unwrap();
        assert_eq!(emu.regs.vx[4], 8);
        assert_eq!(emu.regs.vx[FLAG_REG], 0);

        // Test shift with flag reg (no carry)
        emu.regs.vx[0xf] = 0x3;
        emu.op_shift_left(4, 0xf).unwrap();
        assert_eq!(emu.regs.vx[4], 6);
        assert_eq!(emu.regs.vx[FLAG_REG], 0);

        // Test shift with flag reg (with carry)
        emu.regs.vx[0xf] = 0x81;
        emu.op_shift_left(4, 0xf).unwrap();
        assert_eq!(emu.regs.vx[4], 0x2);
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
            emu.op_load_sprite(0).unwrap();
            assert_eq!(emu.regs.i, DEFAULT_FONT_START + i * DEFAULT_FONT_LEN);
            assert_eq!(
                emu.ram.read_slice(emu.regs.i, DEFAULT_FONT_LEN).unwrap(),
                &DEFAULT_FONT[i as usize]
            );
        }
    }

    #[test]
    fn test_op_call() {
        let mut emu = Emu::new();
        emu.jump(0x200).unwrap();
        emu.op_call(0xfff).unwrap();
        assert_eq!(emu.regs.pc, 0xfff);
        assert_eq!(emu.stack.pop(), Some(0x200));
    }

    #[test]
    fn test_op_store_regs() {
        let mut emu = Emu::new();
        emu.regs.vx[0..16]
            .copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        emu.regs.i = 0;
        emu.op_store_regs(0xf).unwrap();
        for i in 0..16 {
            assert_eq!(emu.ram.read(i).unwrap(), (i + 1) as u8);
        }

        emu.reset();

        emu.regs.vx[0..16]
            .copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        emu.regs.i = 3;
        emu.op_store_regs(4).unwrap();

        assert_eq!(emu.ram.read_slice(emu.regs.i, 5).unwrap(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_op_load_regs() {
        let mut emu = Emu::new();
        emu.ram
            .write_slice(
                0x0,
                &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            )
            .unwrap();
        emu.regs.i = 0;
        emu.op_load_regs(0xf).unwrap();
        for i in 0..16 {
            assert_eq!(emu.regs.vx[i as usize], i + 1);
        }

        emu.reset();

        emu.ram
            .write_slice(
                0x0,
                &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            )
            .unwrap();
        emu.regs.i = 3;
        emu.op_load_regs(4).unwrap();
        for i in 0..4 {
            assert_eq!(emu.regs.vx[i as usize], i + 4);
        }
    }

    #[test]
    fn test_add_index() {
        let mut emu = Emu::new();

        emu.regs.vx[0] = 1;
        emu.op_add_index(0).unwrap();
        assert_eq!(emu.regs.i, 1);

        emu.regs.vx[5] = 4;
        emu.op_add_index(5).unwrap();
        assert_eq!(emu.regs.i, 5);

        // Test overflow
        emu.regs.i = 0xfffe;
        emu.regs.vx[6] = 0x3;
        emu.op_add_index(6).unwrap();
        assert_eq!(emu.regs.i, 1);
    }

    #[test]
    fn test_op_store_bcd() {
        let mut emu = Emu::new();
        emu.regs.vx[0] = 123;
        emu.op_store_bcd(0).unwrap();
        assert_eq!(emu.ram.read_slice(0, 3).unwrap(), &[1, 2, 3]);

        emu.regs.vx[0] = 255;
        emu.op_store_bcd(0).unwrap();
        assert_eq!(emu.ram.read_slice(0, 3).unwrap(), [2, 5, 5]);

        emu.regs.vx[0] = 000;
        emu.op_store_bcd(0).unwrap();
        assert_eq!(emu.ram.read_slice(0, 3).unwrap(), [0, 0, 0]);

        emu.regs.vx[7] = 205;
        emu.regs.i = 0x10;
        emu.op_store_bcd(7).unwrap();
        assert_eq!(emu.ram.read_slice(0x10, 3).unwrap(), [2, 0, 5]);
    }

    #[test]
    fn test_op_jump_plus() {
        let mut emu = Emu::new();
        emu.op_jump_plus(0xfff).unwrap();
        assert_eq!(emu.regs.pc, 0xfff);

        emu.reset();

        emu.regs.vx[0] = 5;
        emu.op_jump_plus(0x005).unwrap();
        assert_eq!(emu.regs.pc, 10);

        emu.reset();

        // The PC is 16bit so it wouldn't overflow on an add like this. It's not super
        // clear how the emu should handle it when the PC is greater than 12 bits (should it
        // ignore the most significant nibble?) but that isn't op_jump_plus's responsibility
        emu.regs.vx[0] = 5;
        emu.op_jump_plus(0xfff).unwrap();
        assert_eq!(emu.regs.pc, 0x1004);
    }
}

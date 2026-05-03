use std::error::Error;
use std::fmt::Display;

const RAM_SIZE: u16 = 0x1000;
const STACK_SIZE: u8 = 16;
const DISPLAY_COLS: u16 = 64;
const DISPLAY_ROWS: u16 = 32;
const RESERVED_END: u16 = 0x1ff;
const UNRESERVED_START: u16 = 0x200;

#[derive(Debug, PartialEq, Eq)]
pub enum EmuError {
    // Invalid address
    AddressError(u16),
    // Unable to load data of size .1 at addr .0 - too large
    LoadError(u16, u16),
    // Invalid instruction .1 at .0
    InvalidInstruction(u16, u16),
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
        }
    }
}

impl Error for EmuError {}

#[derive(Default)]
struct Registers {
    pc: u16,
    sp: u8,
    vx: [u8; 16],
    dt: u8,
    st: u8,
}

pub struct Emu {
    regs: Registers,
    ram: Vec<u8>,
    stack: Vec<u16>,
    frame_buff: Vec<u8>,
}

impl Emu {
    pub fn new() -> Emu {
        Emu {
            regs: Registers::default(),
            ram: vec![0; RAM_SIZE as usize],
            stack: vec![0; STACK_SIZE as usize],
            frame_buff: vec![0; (DISPLAY_COLS * DISPLAY_ROWS) as usize],
        }
    }

    pub fn load(&mut self, addr: u16, data: Vec<u8>) -> Result<(), EmuError> {
        if !is_valid_addr(addr) {
            return Err(EmuError::AddressError(addr));
        }

        if data.len() > (RAM_SIZE - addr) as usize {
            return Err(EmuError::LoadError(addr, data.len() as u16));
        }

        self.ram[(addr as usize)..data.len()].copy_from_slice(&data);

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
        let opcode = u16::from_be_bytes([self.ram[pc as usize], self.ram[(pc + 1) as usize]]);

        match opcode & 0xf000 {
            0 => match opcode & 0x000f {
                0x0 => self.op_cls(opcode),
                0xe => self.op_ret(opcode),
                _ => Err(EmuError::InvalidInstruction(pc, opcode)),
            },
            1 => self.op_jump(opcode),
            2 => self.op_call(opcode),
            3 => self.op_skip_equal_const(opcode),
            4 => self.op_skip_not_equal_const(opcode),
            5 => self.op_skip_equal_reg(opcode),
            6 => self.op_load_const(opcode),
            7 => self.op_add_const(opcode),
            8 => match opcode & 0x000f {
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
            _ => Err(EmuError::InvalidInstruction(pc, opcode)),
        }
    }

    #[inline]
    fn op_ret(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_cls(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_jump(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_call(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_skip_equal_const(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_skip_not_equal_const(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_skip_equal_reg(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_load_const(&mut self, opcode: u16) -> Result<(), EmuError> {
        // TODO:
        Ok(())
    }

    #[inline]
    fn op_add_const(&mut self, opcode: u16) -> Result<(), EmuError> {
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
    fn op_add_reg(&mut self, opcode: u16) -> Result<(), EmuError> {
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
}

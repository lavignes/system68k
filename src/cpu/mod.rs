use self::decoder::{Decoder, EffectiveAddress, Instruction, Size};
use crate::bus::{self, Bus};

mod decoder;

#[cfg(test)]
mod tests;

#[derive(Debug, thiserror::Error)]
enum Exception {
    #[error("address error")]
    AddressError,

    #[error("bus error")]
    BusError(#[from] bus::Error),

    #[error("illegal instruction {0:x}")]
    IllegalInstruction(u16),

    #[error("integer divide by zero")]
    IntegerDivideByZero,

    #[error("privilege violation")]
    PrivilegeViolation,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum Version {
    MC68000 = 0,
    MC68010 = 1,
}

enum StatusFlag {
    Carry = 0x0001,
    Overflow = 0x0002,
    Zero = 0x0004,
    Negative = 0x0008,
    Extend = 0x0010,
    InterruptMask = 0x0700,
    Interrupt = 0x1000,
    Supervisor = 0x2000,
    Tracing = 0x8000,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ComputedEffectiveAddress {
    DataRegister(u8),
    AddressRegister(u8),
    Address(u32),
    Immediate,
}

#[derive(Debug)]
pub struct Cpu {
    version: Version,
    data: [u32; 8],
    addr: [u32; 8],
    pc: u32,

    ssp: u32, // supervisor stack pointer
    sr: u16,  // status register

    caar: u32, // cache access register
    cacr: u32, // cache control register
    dfc: u8,   // destination function code register
    sfc: u8,   // source function code register
    msp: u32,  // master stack pointer register
    vbr: u32,  // vector base register

    decoder: Decoder,
}

impl Cpu {
    pub fn new(version: Version) -> Self {
        Self {
            version,
            data: [0; 8],
            addr: [0; 8],
            pc: 0,
            ssp: 0,
            sr: 0,

            caar: 0,
            cacr: 0,
            dfc: 0,
            sfc: 0,
            msp: 0,
            vbr: 0,

            decoder: Decoder::new(version),
        }
    }

    pub fn reset(&mut self, bus: &mut dyn Bus) {
        self.sr = 0x2700;
        self.ssp = bus.read32(0).unwrap();
        self.pc = bus.read32(4).unwrap();
    }

    #[inline]
    pub fn sr(&self) -> u16 {
        self.sr
    }

    #[inline]
    fn set_sr(&mut self, value: u16) {
        let sr_mask = if self.version <= Version::MC68010 {
            0xA71F
        } else {
            0xF71F
        };
        self.sr = value & sr_mask;
    }

    #[inline]
    fn flag(&self, flag: StatusFlag) -> bool {
        (self.sr & (flag as u16)) != 0
    }

    #[inline]
    fn set_flag(&mut self, flag: StatusFlag, value: bool) {
        if value {
            self.set_sr(self.sr | (flag as u16));
        } else {
            self.set_sr(self.sr & !(flag as u16));
        }
    }

    #[inline]
    fn assert_supervisor(&mut self) -> Result<(), Exception> {
        if !self.flag(StatusFlag::Supervisor) {
            return Err(Exception::PrivilegeViolation);
        }
        Ok(())
    }

    pub fn step(&mut self, bus: &mut dyn Bus) {
        self.decode_execute(bus).unwrap();
    }

    #[inline]
    fn fetch_word(&mut self, bus: &mut dyn Bus) -> Result<u16, Exception> {
        let value = self.read_word(self.pc, bus)?;
        self.pc += 2;
        Ok(value)
    }

    #[inline]
    fn fetch_long(&mut self, bus: &mut dyn Bus) -> Result<u32, Exception> {
        let value = self.read_long(self.pc, bus)?;
        self.pc += 4;
        Ok(value)
    }

    #[inline]
    fn read_byte(&mut self, addr: u32, bus: &mut dyn Bus) -> Result<u8, Exception> {
        Ok(bus.read8(addr)?)
    }

    #[inline]
    fn write_byte(&mut self, addr: u32, value: u8, bus: &mut dyn Bus) -> Result<(), Exception> {
        Ok(bus.write8(addr, value)?)
    }

    #[inline]
    fn read_word(&mut self, addr: u32, bus: &mut dyn Bus) -> Result<u16, Exception> {
        Ok(bus.read16(addr)?)
    }

    #[inline]
    fn write_word(&mut self, addr: u32, value: u16, bus: &mut dyn Bus) -> Result<(), Exception> {
        Ok(bus.write16(addr, value)?)
    }

    #[inline]
    fn read_long(&mut self, addr: u32, bus: &mut dyn Bus) -> Result<u32, Exception> {
        Ok(bus.read32(addr)?)
    }

    #[inline]
    fn write_long(&mut self, addr: u32, value: u32, bus: &mut dyn Bus) -> Result<(), Exception> {
        Ok(bus.write32(addr, value)?)
    }

    fn compute_ea(
        &mut self,
        ea: EffectiveAddress,
        increment: u32,
        bus: &mut dyn Bus,
    ) -> Result<ComputedEffectiveAddress, Exception> {
        match ea {
            EffectiveAddress::DataRegister(register) => {
                Ok(ComputedEffectiveAddress::DataRegister(register))
            }
            EffectiveAddress::AddressRegister(register) => {
                Ok(ComputedEffectiveAddress::AddressRegister(register))
            }
            EffectiveAddress::Address(register) => Ok(ComputedEffectiveAddress::Address(
                self.addr[register as usize],
            )),
            EffectiveAddress::AddressWithPostIncrement(register) => {
                let addr = self.addr[register as usize];
                if (register == 7) && (increment == 1) {
                    self.addr[7] = self.addr[7].wrapping_add(2);
                } else {
                    self.addr[register as usize] = self.addr[register as usize].wrapping_add(2);
                }
                Ok(ComputedEffectiveAddress::Address(addr))
            }
            EffectiveAddress::AddressWithPreDecrement(register) => {
                if (register == 7) && (increment == 1) {
                    self.addr[7] = self.addr[7].wrapping_sub(2);
                } else {
                    self.addr[register as usize] = self.addr[register as usize].wrapping_sub(2);
                }
                Ok(ComputedEffectiveAddress::Address(
                    self.addr[register as usize],
                ))
            }
            EffectiveAddress::AddressWithDisplacement(register) => {
                // TODO: can I get away with converting back to u32?
                let displacement = ((self.fetch_word(bus)? as i16) as i32) as u32;
                Ok(ComputedEffectiveAddress::Address(
                    self.addr[register as usize].wrapping_add(displacement),
                ))
            }
            EffectiveAddress::AddressWithIndex(register) => todo!(),
            EffectiveAddress::PcWithDisplacement => {
                let pc = self.pc;
                // TODO: can I get away with converting back to u32?
                let displacement = ((self.fetch_word(bus)? as i16) as i32) as u32;
                Ok(ComputedEffectiveAddress::Address(
                    pc.wrapping_add(displacement),
                ))
            }
            EffectiveAddress::PcWithIndex => todo!(),
            EffectiveAddress::AbsoluteShort => {
                Ok(ComputedEffectiveAddress::Address(self.fetch_long(bus)?))
            }
            EffectiveAddress::AbsoluteLong => {
                Ok(ComputedEffectiveAddress::Address(self.fetch_long(bus)?))
            }
            EffectiveAddress::Immediate => Ok(ComputedEffectiveAddress::Immediate),
        }
    }

    #[inline]
    fn read_ea_byte(
        &mut self,
        ea: ComputedEffectiveAddress,
        bus: &mut dyn Bus,
    ) -> Result<u8, Exception> {
        match ea {
            ComputedEffectiveAddress::DataRegister(register) => {
                Ok(self.data[register as usize] as u8)
            }
            ComputedEffectiveAddress::AddressRegister(_) => unreachable!(),
            ComputedEffectiveAddress::Address(addr) => self.read_byte(addr, bus),
            ComputedEffectiveAddress::Immediate => Ok(self.fetch_word(bus)? as u8),
        }
    }

    #[inline]
    fn read_ea_word(
        &mut self,
        ea: ComputedEffectiveAddress,
        bus: &mut dyn Bus,
    ) -> Result<u16, Exception> {
        match ea {
            ComputedEffectiveAddress::DataRegister(register) => {
                Ok(self.data[register as usize] as u16)
            }
            ComputedEffectiveAddress::AddressRegister(_) => unreachable!(),
            ComputedEffectiveAddress::Address(addr) => self.read_word(addr, bus),
            ComputedEffectiveAddress::Immediate => Ok(self.fetch_word(bus)?),
        }
    }

    #[inline]
    fn read_ea_long(
        &mut self,
        ea: ComputedEffectiveAddress,
        bus: &mut dyn Bus,
    ) -> Result<u32, Exception> {
        match ea {
            ComputedEffectiveAddress::DataRegister(register) => Ok(self.data[register as usize]),
            ComputedEffectiveAddress::AddressRegister(register) => Ok(self.addr[register as usize]),
            ComputedEffectiveAddress::Address(addr) => self.read_long(addr, bus),
            ComputedEffectiveAddress::Immediate => Ok(self.fetch_long(bus)?),
        }
    }

    #[inline]
    fn write_ea_byte(
        &mut self,
        ea: ComputedEffectiveAddress,
        value: u8,
        bus: &mut dyn Bus,
    ) -> Result<(), Exception> {
        match ea {
            ComputedEffectiveAddress::DataRegister(register) => {
                self.data[register as usize] =
                    (self.data[register as usize] & 0xFFFFFF00) | (value as u32);
                Ok(())
            }
            ComputedEffectiveAddress::AddressRegister(_) => unreachable!(),
            ComputedEffectiveAddress::Address(addr) => self.write_byte(addr, value, bus),
            ComputedEffectiveAddress::Immediate => unreachable!(),
        }
    }

    #[inline]
    fn write_ea_word(
        &mut self,
        ea: ComputedEffectiveAddress,
        value: u16,
        bus: &mut dyn Bus,
    ) -> Result<(), Exception> {
        match ea {
            ComputedEffectiveAddress::DataRegister(register) => {
                self.data[register as usize] =
                    (self.data[register as usize] & 0xFFFF0000) | (value as u32);
                Ok(())
            }
            ComputedEffectiveAddress::AddressRegister(_) => unreachable!(),
            ComputedEffectiveAddress::Address(addr) => self.write_word(addr, value, bus),
            ComputedEffectiveAddress::Immediate => unreachable!(),
        }
    }

    #[inline]
    fn write_ea_long(
        &mut self,
        ea: ComputedEffectiveAddress,
        value: u32,
        bus: &mut dyn Bus,
    ) -> Result<(), Exception> {
        match ea {
            ComputedEffectiveAddress::DataRegister(register) => {
                self.data[register as usize] = value;
                Ok(())
            }
            ComputedEffectiveAddress::AddressRegister(register) => {
                self.addr[register as usize] = value;
                Ok(())
            }
            ComputedEffectiveAddress::Address(addr) => self.write_long(addr, value, bus),
            ComputedEffectiveAddress::Immediate => unreachable!(),
        }
    }

    fn decode_execute(&mut self, bus: &mut dyn Bus) -> Result<(), Exception> {
        let opcode = self.fetch_word(bus)?;

        match self.decoder.decode(opcode) {
            Instruction::Illegal => Err(Exception::IllegalInstruction(opcode)),

            Instruction::OriToCcr => {
                let value = self.fetch_word(bus)?;
                let ccr = self.sr & 0x00FF;
                self.set_sr((self.sr & 0xFF00) | (ccr | (value & 0x00FF)));
                Ok(())
            }

            Instruction::OriToSr => {
                self.assert_supervisor()?;
                let value = self.fetch_word(bus)?;
                self.set_sr(self.sr | value);
                Ok(())
            }

            Instruction::Ori(size, ea) => match size {
                Size::Byte => {
                    let ea = self.compute_ea(ea, 1, bus)?;
                    let lhs = self.read_ea_byte(ea, bus)?;
                    let imm = self.fetch_word(bus)? as u8;
                    let result = lhs | imm;
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80) != 0);
                    self.set_flag(StatusFlag::Carry, false);
                    self.set_flag(StatusFlag::Overflow, false);
                    self.write_ea_byte(ea, result, bus)
                }

                Size::Word => {
                    let ea = self.compute_ea(ea, 2, bus)?;
                    let lhs = self.read_ea_word(ea, bus)?;
                    let imm = self.fetch_word(bus)?;
                    let result = lhs | imm;
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x8000) != 0);
                    self.set_flag(StatusFlag::Carry, false);
                    self.set_flag(StatusFlag::Overflow, false);
                    self.write_ea_word(ea, result, bus)
                }

                Size::Long => {
                    let ea = self.compute_ea(ea, 4, bus)?;
                    let lhs = self.read_ea_long(ea, bus)?;
                    let imm = self.fetch_long(bus)?;
                    let result = lhs | imm;
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80000000) != 0);
                    self.set_flag(StatusFlag::Carry, false);
                    self.set_flag(StatusFlag::Overflow, false);
                    self.write_ea_long(ea, result, bus)
                }
            },

            Instruction::AndiToCcr => {
                let value = self.fetch_word(bus)?;
                let ccr = self.sr & 0x00FF;
                self.set_sr((self.sr & 0xFF00) | (ccr & (value & 0x00FF)));
                Ok(())
            }

            Instruction::AndiToSr => {
                self.assert_supervisor()?;
                let value = self.fetch_word(bus)?;
                self.set_sr(self.sr & value);
                Ok(())
            }

            Instruction::Andi(size, ea) => match size {
                Size::Byte => {
                    let ea = self.compute_ea(ea, 1, bus)?;
                    let lhs = self.read_ea_byte(ea, bus)?;
                    let imm = self.fetch_word(bus)? as u8;
                    let result = lhs & imm;
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80) != 0);
                    self.set_flag(StatusFlag::Carry, false);
                    self.set_flag(StatusFlag::Overflow, false);
                    self.write_ea_byte(ea, result, bus)
                }

                Size::Word => {
                    let ea = self.compute_ea(ea, 2, bus)?;
                    let lhs = self.read_ea_word(ea, bus)?;
                    let imm = self.fetch_word(bus)?;
                    let result = lhs & imm;
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x8000) != 0);
                    self.set_flag(StatusFlag::Carry, false);
                    self.set_flag(StatusFlag::Overflow, false);
                    self.write_ea_word(ea, result, bus)
                }

                Size::Long => {
                    let ea = self.compute_ea(ea, 4, bus)?;
                    let lhs = self.read_ea_long(ea, bus)?;
                    let imm = self.fetch_long(bus)?;
                    let result = lhs & imm;
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80000000) != 0);
                    self.set_flag(StatusFlag::Carry, false);
                    self.set_flag(StatusFlag::Overflow, false);
                    self.write_ea_long(ea, result, bus)
                }
            },

            Instruction::Subi(size, ea) => match size {
                Size::Byte => {
                    let ea = self.compute_ea(ea, 1, bus)?;
                    let lhs = self.read_ea_byte(ea, bus)?;
                    let imm = self.fetch_word(bus)? as u8;
                    let (result, borrow) = lhs.borrowing_sub(imm, false);
                    let overflow = lhs.checked_sub(imm).is_none();
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80) != 0);
                    self.set_flag(StatusFlag::Carry, borrow);
                    self.set_flag(StatusFlag::Extend, borrow);
                    self.set_flag(StatusFlag::Overflow, overflow);
                    self.write_ea_byte(ea, result, bus)
                }

                Size::Word => {
                    let ea = self.compute_ea(ea, 2, bus)?;
                    let lhs = self.read_ea_word(ea, bus)?;
                    let imm = self.fetch_word(bus)?;
                    let (result, borrow) = lhs.borrowing_sub(imm, false);
                    let overflow = lhs.checked_sub(imm).is_none();
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x8000) != 0);
                    self.set_flag(StatusFlag::Carry, borrow);
                    self.set_flag(StatusFlag::Extend, borrow);
                    self.set_flag(StatusFlag::Overflow, overflow);
                    self.write_ea_word(ea, result, bus)
                }

                Size::Long => {
                    let ea = self.compute_ea(ea, 4, bus)?;
                    let lhs = self.read_ea_long(ea, bus)?;
                    let imm = self.fetch_long(bus)?;
                    let (result, borrow) = lhs.borrowing_sub(imm, false);
                    let overflow = lhs.checked_sub(imm).is_none();
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80000000) != 0);
                    self.set_flag(StatusFlag::Carry, borrow);
                    self.set_flag(StatusFlag::Extend, borrow);
                    self.set_flag(StatusFlag::Overflow, overflow);
                    self.write_ea_long(ea, result, bus)
                }
            },

            Instruction::Addi(size, ea) => match size {
                Size::Byte => {
                    let ea = self.compute_ea(ea, 1, bus)?;
                    let lhs = self.read_ea_byte(ea, bus)?;
                    let imm = self.fetch_word(bus)? as u8;
                    let (result, carry) = lhs.carrying_add(imm, false);
                    let overflow = lhs.checked_add(imm).is_none();
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80) != 0);
                    self.set_flag(StatusFlag::Carry, carry);
                    self.set_flag(StatusFlag::Extend, carry);
                    self.set_flag(StatusFlag::Overflow, overflow);
                    self.write_ea_byte(ea, result, bus)
                }

                Size::Word => {
                    let ea = self.compute_ea(ea, 2, bus)?;
                    let lhs = self.read_ea_word(ea, bus)?;
                    let imm = self.fetch_word(bus)?;
                    let (result, carry) = lhs.carrying_add(imm, false);
                    let overflow = lhs.checked_add(imm).is_none();
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x8000) != 0);
                    self.set_flag(StatusFlag::Carry, carry);
                    self.set_flag(StatusFlag::Extend, carry);
                    self.set_flag(StatusFlag::Overflow, overflow);
                    self.write_ea_word(ea, result, bus)
                }

                Size::Long => {
                    let ea = self.compute_ea(ea, 4, bus)?;
                    let lhs = self.read_ea_long(ea, bus)?;
                    let imm = self.fetch_long(bus)?;
                    let (result, carry) = lhs.carrying_add(imm, false);
                    let overflow = lhs.checked_add(imm).is_none();
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80000000) != 0);
                    self.set_flag(StatusFlag::Carry, carry);
                    self.set_flag(StatusFlag::Extend, carry);
                    self.set_flag(StatusFlag::Overflow, overflow);
                    self.write_ea_long(ea, result, bus)
                }
            },

            Instruction::EoriToCcr => {
                let value = self.fetch_word(bus)?;
                let ccr = self.sr & 0x00FF;
                self.set_sr((self.sr & 0xFF00) | (ccr ^ (value & 0x00FF)));
                Ok(())
            }

            Instruction::EoriToSr => {
                self.assert_supervisor()?;
                let value = self.fetch_word(bus)?;
                self.set_sr(self.sr ^ value);
                Ok(())
            }

            Instruction::Eori(size, ea) => match size {
                Size::Byte => {
                    let ea = self.compute_ea(ea, 1, bus)?;
                    let lhs = self.read_ea_byte(ea, bus)?;
                    let imm = self.fetch_word(bus)? as u8;
                    let result = lhs ^ imm;
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80) != 0);
                    self.set_flag(StatusFlag::Carry, false);
                    self.set_flag(StatusFlag::Overflow, false);
                    self.write_ea_byte(ea, result, bus)
                }

                Size::Word => {
                    let ea = self.compute_ea(ea, 2, bus)?;
                    let lhs = self.read_ea_word(ea, bus)?;
                    let imm = self.fetch_word(bus)?;
                    let result = lhs ^ imm;
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x8000) != 0);
                    self.set_flag(StatusFlag::Carry, false);
                    self.set_flag(StatusFlag::Overflow, false);
                    self.write_ea_word(ea, result, bus)
                }

                Size::Long => {
                    let ea = self.compute_ea(ea, 4, bus)?;
                    let lhs = self.read_ea_long(ea, bus)?;
                    let imm = self.fetch_long(bus)?;
                    let result = lhs ^ imm;
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80000000) != 0);
                    self.set_flag(StatusFlag::Carry, false);
                    self.set_flag(StatusFlag::Overflow, false);
                    self.write_ea_long(ea, result, bus)
                }
            },

            Instruction::Cmpi(size, ea) => match size {
                Size::Byte => {
                    let ea = self.compute_ea(ea, 1, bus)?;
                    let lhs = self.read_ea_byte(ea, bus)?;
                    let imm = self.fetch_word(bus)? as u8;
                    let (result, borrow) = lhs.borrowing_sub(imm, false);
                    let overflow = lhs.checked_sub(imm).is_none();
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80) != 0);
                    self.set_flag(StatusFlag::Extend, borrow);
                    self.set_flag(StatusFlag::Overflow, overflow);
                    Ok(())
                }

                Size::Word => {
                    let ea = self.compute_ea(ea, 2, bus)?;
                    let lhs = self.read_ea_word(ea, bus)?;
                    let imm = self.fetch_word(bus)?;
                    let (result, borrow) = lhs.borrowing_sub(imm, false);
                    let overflow = lhs.checked_sub(imm).is_none();
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x8000) != 0);
                    self.set_flag(StatusFlag::Extend, borrow);
                    self.set_flag(StatusFlag::Overflow, overflow);
                    Ok(())
                }

                Size::Long => {
                    let ea = self.compute_ea(ea, 4, bus)?;
                    let lhs = self.read_ea_long(ea, bus)?;
                    let imm = self.fetch_long(bus)?;
                    let (result, borrow) = lhs.borrowing_sub(imm, false);
                    let overflow = lhs.checked_sub(imm).is_none();
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80000000) != 0);
                    self.set_flag(StatusFlag::Extend, borrow);
                    self.set_flag(StatusFlag::Overflow, overflow);
                    Ok(())
                }
            },

            Instruction::Btst(register, ea) => {
                let ea = self.compute_ea(ea, 1, bus)?;
                let (value, mask) = if let ComputedEffectiveAddress::DataRegister(register) = ea {
                    (self.data[register as usize], 0b11111)
                } else {
                    (self.read_ea_byte(ea, bus)? as u32, 0b111)
                };
                let bit = match register {
                    Some(register) => self.data[register as usize] & mask,
                    None => (self.fetch_word(bus)? as u32) & mask,
                };
                self.set_flag(StatusFlag::Zero, ((1 << bit) & value) == 0);
                Ok(())
            }

            Instruction::Bchg(register, ea) => {
                let ea = self.compute_ea(ea, 1, bus)?;
                let (value, mask) = if let ComputedEffectiveAddress::DataRegister(register) = ea {
                    (self.data[register as usize], 0b11111)
                } else {
                    (self.read_ea_byte(ea, bus)? as u32, 0b111)
                };
                let bit = match register {
                    Some(register) => self.data[register as usize] & mask,
                    None => (self.fetch_word(bus)? as u32) & mask,
                };
                self.set_flag(StatusFlag::Zero, ((1 << bit) & value) == 0);
                let value = value ^ (1 << bit);
                if let ComputedEffectiveAddress::DataRegister(_) = ea {
                    self.write_ea_long(ea, value, bus)
                } else {
                    self.write_ea_byte(ea, value as u8, bus)
                }
            }

            Instruction::Bclr(register, ea) => {
                let ea = self.compute_ea(ea, 1, bus)?;
                let (value, mask) = if let ComputedEffectiveAddress::DataRegister(register) = ea {
                    (self.data[register as usize], 0b11111)
                } else {
                    (self.read_ea_byte(ea, bus)? as u32, 0b111)
                };
                let bit = match register {
                    Some(register) => self.data[register as usize] & mask,
                    None => (self.fetch_word(bus)? as u32) & mask,
                };
                self.set_flag(StatusFlag::Zero, ((1 << bit) & value) == 0);
                let value = value & !(1 << bit);
                if let ComputedEffectiveAddress::DataRegister(_) = ea {
                    self.write_ea_long(ea, value, bus)
                } else {
                    self.write_ea_byte(ea, value as u8, bus)
                }
            }

            Instruction::Bset(register, ea) => {
                let ea = self.compute_ea(ea, 1, bus)?;
                let (value, mask) = if let ComputedEffectiveAddress::DataRegister(register) = ea {
                    (self.data[register as usize], 0b11111)
                } else {
                    (self.read_ea_byte(ea, bus)? as u32, 0b111)
                };
                let bit = match register {
                    Some(register) => self.data[register as usize] & mask,
                    None => (self.fetch_word(bus)? as u32) & mask,
                };
                self.set_flag(StatusFlag::Zero, ((1 << bit) & value) == 0);
                let value = value | (1 << bit);
                if let ComputedEffectiveAddress::DataRegister(_) = ea {
                    self.write_ea_long(ea, value, bus)
                } else {
                    self.write_ea_byte(ea, value as u8, bus)
                }
            }

            Instruction::Movep(_, _, _, _) => todo!("MOVEP not implemented yet! :("),

            Instruction::Moveq(data, register) => {
                // sign extend
                let value = ((data as i8) as i32) as u32;
                self.data[register as usize] = value;
                self.set_flag(StatusFlag::Zero, value == 0);
                self.set_flag(StatusFlag::Negative, (value & 0x80000000) != 0);
                self.set_flag(StatusFlag::Overflow, false);
                self.set_flag(StatusFlag::Carry, false);
                Ok(())
            }

            _ => todo!(),
        }
    }
}

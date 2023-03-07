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
    data: [u32; 8],
    addr: [u32; 7],
    pc: u32,
    usp: u32, // user stack pointer
    ssp: u32, // supervisor stack pointer
    sr: u16,  // status register

    decoder: Decoder,

    is_stopped: bool,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            data: [0; 8],
            addr: [0; 7],
            pc: 0,
            usp: 0,
            ssp: 0,
            sr: 0,

            decoder: Decoder::new(),

            is_stopped: false,
        }
    }

    pub fn reset(&mut self, bus: &mut dyn Bus) {
        self.sr = 0x2700;
        self.ssp = bus.read32(0).unwrap();
        self.pc = bus.read32(4).unwrap();
    }

    #[inline]
    pub fn data(&self, register: usize) -> u32 {
        self.data[register]
    }

    #[inline]
    pub fn set_data(&mut self, register: usize, value: u32) {
        self.data[register] = value
    }

    #[inline]
    pub fn addr(&self, register: usize) -> u32 {
        if register == 7 {
            if self.flag(StatusFlag::Supervisor) {
                self.ssp
            } else {
                self.usp
            }
        } else {
            self.addr[register]
        }
    }

    #[inline]
    pub fn set_addr(&mut self, register: usize, value: u32) {
        if register == 7 {
            if self.flag(StatusFlag::Supervisor) {
                self.ssp = value;
            } else {
                self.usp = value;
            }
        } else {
            self.addr[register] = value;
        }
    }

    #[inline]
    pub fn pc(&self) -> u32 {
        self.pc
    }

    #[inline]
    pub fn set_pc(&mut self, value: u32) {
        self.pc = value;
    }

    #[inline]
    pub fn sr(&self) -> u16 {
        self.sr
    }

    #[inline]
    pub fn set_sr(&mut self, value: u16) {
        self.sr = value & 0xF71f;
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

    #[inline]
    pub fn step(&mut self, bus: &mut dyn Bus) {
        self.decode_execute(bus).unwrap();
    }

    #[inline]
    pub fn is_stopped(&self) -> bool {
        self.is_stopped
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
            EffectiveAddress::Address(register) => {
                Ok(ComputedEffectiveAddress::Address(if register == 7 {
                    if self.flag(StatusFlag::Supervisor) {
                        self.ssp
                    } else {
                        self.usp
                    }
                } else {
                    self.addr[register as usize]
                }))
            }
            EffectiveAddress::AddressWithPostIncrement(register) => {
                Ok(ComputedEffectiveAddress::Address(if register == 7 {
                    if self.flag(StatusFlag::Supervisor) {
                        let addr = self.ssp;
                        self.ssp =
                            self.ssp
                                .wrapping_add(if increment == 1 { 2 } else { increment });
                        addr
                    } else {
                        let addr = self.usp;
                        self.usp =
                            self.usp
                                .wrapping_add(if increment == 1 { 2 } else { increment });
                        addr
                    }
                } else {
                    let addr = self.addr[register as usize];
                    self.addr[register as usize] =
                        self.addr[register as usize].wrapping_add(increment);
                    addr
                }))
            }
            EffectiveAddress::AddressWithPreDecrement(register) => {
                Ok(ComputedEffectiveAddress::Address(if register == 7 {
                    if self.flag(StatusFlag::Supervisor) {
                        self.ssp =
                            self.ssp
                                .wrapping_sub(if increment == 1 { 2 } else { increment });
                        self.ssp
                    } else {
                        self.usp =
                            self.usp
                                .wrapping_sub(if increment == 1 { 2 } else { increment });
                        self.usp
                    }
                } else {
                    self.addr[register as usize] =
                        self.addr[register as usize].wrapping_sub(increment);
                    self.addr[register as usize]
                }))
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
            EffectiveAddress::AbsoluteShort => Ok(ComputedEffectiveAddress::Address(
                self.fetch_word(bus)? as u32,
            )),
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
            ComputedEffectiveAddress::AddressRegister(register) => Ok(if register == 7 {
                if self.flag(StatusFlag::Supervisor) {
                    self.ssp
                } else {
                    self.usp
                }
            } else {
                self.addr[register as usize]
            }),
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
                if register == 7 {
                    if self.flag(StatusFlag::Supervisor) {
                        self.ssp = value;
                    } else {
                        self.usp = value;
                    }
                    Ok(())
                } else {
                    self.addr[register as usize] = value;
                    Ok(())
                }
            }
            ComputedEffectiveAddress::Address(addr) => self.write_long(addr, value, bus),
            ComputedEffectiveAddress::Immediate => unreachable!(),
        }
    }

    #[inline]
    fn push_word(&mut self, value: u32, bus: &mut dyn Bus) -> Result<(), Exception> {
        if self.flag(StatusFlag::Supervisor) {
            self.ssp = self.ssp.wrapping_sub(2);
            self.write_long(self.ssp, value, bus)
        } else {
            self.usp = self.usp.wrapping_sub(2);
            self.write_long(self.usp, value, bus)
        }
    }

    #[inline]
    fn push_long(&mut self, value: u32, bus: &mut dyn Bus) -> Result<(), Exception> {
        if self.flag(StatusFlag::Supervisor) {
            self.ssp = self.ssp.wrapping_sub(4);
            self.write_long(self.ssp, value, bus)
        } else {
            self.usp = self.usp.wrapping_sub(4);
            self.write_long(self.usp, value, bus)
        }
    }

    fn decode_execute(&mut self, bus: &mut dyn Bus) -> Result<(), Exception> {
        let opcode = self.fetch_word(bus)?;

        match self.decoder.decode(opcode) {
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

            Instruction::Movea(size, ea, register) => match size {
                Size::Word => {
                    let ea = self.compute_ea(ea, 2, bus)?;
                    let value = self.read_ea_word(ea, bus)? as u32;
                    if register == 7 {
                        if self.flag(StatusFlag::Supervisor) {
                            self.ssp = (self.ssp & 0xFFFF0000) | value;
                        } else {
                            self.usp = (self.usp & 0xFFFF0000) | value;
                        }
                    } else {
                        self.addr[register as usize] =
                            (self.addr[register as usize] & 0xFFFF0000) | value;
                    }
                    Ok(())
                }

                Size::Long => {
                    let ea = self.compute_ea(ea, 4, bus)?;
                    let value = self.read_ea_long(ea, bus)?;
                    if register == 7 {
                        if self.flag(StatusFlag::Supervisor) {
                            self.ssp = value;
                        } else {
                            self.usp = value;
                        }
                    } else {
                        self.addr[register as usize] = value;
                    }
                    Ok(())
                }

                _ => unreachable!(),
            },

            Instruction::Move(size, src, dst) => match size {
                Size::Byte => {
                    let src = self.compute_ea(src, 1, bus)?;
                    let value = self.read_ea_byte(src, bus)?;
                    self.set_flag(StatusFlag::Zero, value == 0);
                    self.set_flag(StatusFlag::Negative, (value & 0x80) == 0x80);
                    self.set_flag(StatusFlag::Carry, false);
                    self.set_flag(StatusFlag::Overflow, false);
                    let dst = self.compute_ea(dst, 1, bus)?;
                    self.write_ea_byte(dst, value, bus)
                }

                Size::Word => {
                    let src = self.compute_ea(src, 2, bus)?;
                    let value = self.read_ea_word(src, bus)?;
                    self.set_flag(StatusFlag::Zero, value == 0);
                    self.set_flag(StatusFlag::Negative, (value & 0x8000) == 0x8000);
                    self.set_flag(StatusFlag::Carry, false);
                    self.set_flag(StatusFlag::Overflow, false);
                    let dst = self.compute_ea(dst, 2, bus)?;
                    self.write_ea_word(dst, value, bus)
                }

                Size::Long => {
                    let src = self.compute_ea(src, 4, bus)?;
                    let value = self.read_ea_long(src, bus)?;
                    self.set_flag(StatusFlag::Zero, value == 0);
                    self.set_flag(StatusFlag::Negative, (value & 0x80000000) == 0x80000000);
                    self.set_flag(StatusFlag::Carry, false);
                    self.set_flag(StatusFlag::Overflow, false);
                    let dst = self.compute_ea(dst, 4, bus)?;
                    self.write_ea_long(dst, value, bus)
                }
            },

            Instruction::MoveFromSr(ea) => {
                self.assert_supervisor()?;
                let ea = self.compute_ea(ea, 2, bus)?;
                self.write_ea_word(ea, self.sr, bus)
            }

            Instruction::MoveToCcr(ea) => {
                let ea = self.compute_ea(ea, 1, bus)?;
                let value = self.read_ea_byte(ea, bus)? as u16;
                self.set_sr((self.sr & 0xFF00) | value);
                Ok(())
            }

            Instruction::MoveToSr(ea) => {
                self.assert_supervisor()?;
                let ea = self.compute_ea(ea, 2, bus)?;
                let value = self.read_ea_word(ea, bus)?;
                self.set_sr(value);
                Ok(())
            }

            Instruction::Negx(size, ea) => match size {
                Size::Byte => {
                    let ea = self.compute_ea(ea, 1, bus)?;
                    let value = self.read_ea_byte(ea, bus)?;
                    let (result, borrow) = 0u8.borrowing_sub(value, self.flag(StatusFlag::Extend));
                    let overflow = if let Some(result) = 0u8.checked_sub(value) {
                        result
                            .checked_sub(if self.flag(StatusFlag::Extend) { 0 } else { 1 })
                            .is_none()
                    } else {
                        true
                    };
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80) != 0);
                    self.set_flag(StatusFlag::Carry, borrow);
                    self.set_flag(StatusFlag::Extend, borrow);
                    self.set_flag(StatusFlag::Overflow, overflow);
                    self.write_ea_byte(ea, result, bus)
                }

                Size::Word => {
                    let ea = self.compute_ea(ea, 1, bus)?;
                    let value = self.read_ea_word(ea, bus)?;
                    let (result, borrow) = 0u16.borrowing_sub(value, self.flag(StatusFlag::Extend));
                    let overflow = if let Some(result) = 0u16.checked_sub(value) {
                        result
                            .checked_sub(if self.flag(StatusFlag::Extend) { 0 } else { 1 })
                            .is_none()
                    } else {
                        true
                    };
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x8000) != 0);
                    self.set_flag(StatusFlag::Carry, borrow);
                    self.set_flag(StatusFlag::Extend, borrow);
                    self.set_flag(StatusFlag::Overflow, overflow);
                    self.write_ea_word(ea, result, bus)
                }

                Size::Long => {
                    let ea = self.compute_ea(ea, 1, bus)?;
                    let value = self.read_ea_long(ea, bus)?;
                    let (result, borrow) = 0u32.borrowing_sub(value, self.flag(StatusFlag::Extend));
                    let overflow = if let Some(result) = 0u32.checked_sub(value) {
                        result
                            .checked_sub(if self.flag(StatusFlag::Extend) { 0 } else { 1 })
                            .is_none()
                    } else {
                        true
                    };
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80000000) != 0);
                    self.set_flag(StatusFlag::Carry, borrow);
                    self.set_flag(StatusFlag::Extend, borrow);
                    self.set_flag(StatusFlag::Overflow, overflow);
                    self.write_ea_long(ea, result, bus)
                }
            },

            Instruction::Clr(size, ea) => match size {
                Size::Byte => {
                    let ea = self.compute_ea(ea, 1, bus)?;
                    self.set_flag(StatusFlag::Zero, true);
                    self.set_flag(StatusFlag::Negative, false);
                    self.set_flag(StatusFlag::Carry, false);
                    self.set_flag(StatusFlag::Overflow, false);
                    self.write_ea_byte(ea, 0, bus)
                }

                Size::Word => {
                    let ea = self.compute_ea(ea, 2, bus)?;
                    self.set_flag(StatusFlag::Zero, true);
                    self.set_flag(StatusFlag::Negative, false);
                    self.set_flag(StatusFlag::Carry, false);
                    self.set_flag(StatusFlag::Overflow, false);
                    self.write_ea_word(ea, 0, bus)
                }

                Size::Long => {
                    let ea = self.compute_ea(ea, 4, bus)?;
                    self.set_flag(StatusFlag::Zero, true);
                    self.set_flag(StatusFlag::Negative, false);
                    self.set_flag(StatusFlag::Carry, false);
                    self.set_flag(StatusFlag::Overflow, false);
                    self.write_ea_long(ea, 0, bus)
                }
            },

            Instruction::Neg(size, ea) => match size {
                Size::Byte => {
                    let ea = self.compute_ea(ea, 1, bus)?;
                    let value = self.read_ea_byte(ea, bus)?;
                    let (result, borrow) = 0u8.borrowing_sub(value, false);
                    let overflow = 0u8.checked_sub(value).is_none();
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80) != 0);
                    self.set_flag(StatusFlag::Carry, borrow);
                    self.set_flag(StatusFlag::Extend, borrow);
                    self.set_flag(StatusFlag::Overflow, overflow);
                    self.write_ea_byte(ea, result, bus)
                }

                Size::Word => {
                    let ea = self.compute_ea(ea, 1, bus)?;
                    let value = self.read_ea_word(ea, bus)?;
                    let (result, borrow) = 0u16.borrowing_sub(value, false);
                    let overflow = 0u16.checked_sub(value).is_none();
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x8000) != 0);
                    self.set_flag(StatusFlag::Carry, borrow);
                    self.set_flag(StatusFlag::Extend, borrow);
                    self.set_flag(StatusFlag::Overflow, overflow);
                    self.write_ea_word(ea, result, bus)
                }

                Size::Long => {
                    let ea = self.compute_ea(ea, 1, bus)?;
                    let value = self.read_ea_long(ea, bus)?;
                    let (result, borrow) = 0u32.borrowing_sub(value, false);
                    let overflow = 0u32.checked_sub(value).is_none();
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80000000) != 0);
                    self.set_flag(StatusFlag::Carry, borrow);
                    self.set_flag(StatusFlag::Extend, borrow);
                    self.set_flag(StatusFlag::Overflow, overflow);
                    self.write_ea_long(ea, result, bus)
                }
            },

            Instruction::Not(size, ea) => match size {
                Size::Byte => {
                    let ea = self.compute_ea(ea, 1, bus)?;
                    let value = self.read_ea_byte(ea, bus)?;
                    let result = !value;
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80) != 0);
                    self.set_flag(StatusFlag::Overflow, false);
                    self.set_flag(StatusFlag::Carry, false);
                    self.write_ea_byte(ea, result, bus)
                }

                Size::Word => {
                    let ea = self.compute_ea(ea, 2, bus)?;
                    let value = self.read_ea_word(ea, bus)?;
                    let result = !value;
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x8000) != 0);
                    self.set_flag(StatusFlag::Overflow, false);
                    self.set_flag(StatusFlag::Carry, false);
                    self.write_ea_word(ea, result, bus)
                }

                Size::Long => {
                    let ea = self.compute_ea(ea, 4, bus)?;
                    let value = self.read_ea_long(ea, bus)?;
                    let result = !value;
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80000000) != 0);
                    self.set_flag(StatusFlag::Overflow, false);
                    self.set_flag(StatusFlag::Carry, false);
                    self.write_ea_long(ea, result, bus)
                }
            },

            Instruction::Ext(size, register) => match size {
                Size::Word => {
                    let result = (((self.data[register as usize] as u8) as i8) as i16) as u16;
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x8000) != 0);
                    self.set_flag(StatusFlag::Overflow, false);
                    self.set_flag(StatusFlag::Carry, false);
                    self.data[register as usize] =
                        (self.data[register as usize] & 0xFFFF0000) | (result as u32);
                    Ok(())
                }

                Size::Long => {
                    let result = (((self.data[register as usize] as u16) as i16) as i32) as u32;
                    self.set_flag(StatusFlag::Zero, result == 0);
                    self.set_flag(StatusFlag::Negative, (result & 0x80000000) != 0);
                    self.set_flag(StatusFlag::Overflow, false);
                    self.set_flag(StatusFlag::Carry, false);
                    self.data[register as usize] = result;
                    Ok(())
                }

                _ => unreachable!(),
            },

            Instruction::Nbcd(_) => todo!("NBCD not implemented yet! :("),

            Instruction::Swap(register) => {
                let value = self.data[register as usize];
                let result = (value << 16) | (value >> 16);
                self.data[register as usize] = result;
                self.set_flag(StatusFlag::Zero, result == 0);
                self.set_flag(StatusFlag::Negative, (result & 0x80000000) != 0);
                self.set_flag(StatusFlag::Overflow, false);
                self.set_flag(StatusFlag::Carry, false);
                Ok(())
            }

            Instruction::Pea(ea) => {
                let ea = self.compute_ea(ea, 4, bus)?;
                let value = self.read_ea_long(ea, bus)?;
                self.push_long(value, bus)
            }

            Instruction::Illegal => Err(Exception::IllegalInstruction(opcode)),

            Instruction::Tas(ea) => {
                let ea = self.compute_ea(ea, 1, bus)?;
                let value = self.read_ea_byte(ea, bus)?;
                self.set_flag(StatusFlag::Zero, value == 0);
                self.set_flag(StatusFlag::Negative, (value & 0x80) != 0);
                self.set_flag(StatusFlag::Overflow, false);
                self.set_flag(StatusFlag::Carry, false);
                self.write_ea_byte(ea, value | 0x80, bus)
            }

            Instruction::Moveq(data, register) => {
                // sign extend
                let result = ((data as i8) as i32) as u32;
                self.data[register as usize] = result;
                self.set_flag(StatusFlag::Zero, result == 0);
                self.set_flag(StatusFlag::Negative, (result & 0x80000000) != 0);
                self.set_flag(StatusFlag::Overflow, false);
                self.set_flag(StatusFlag::Carry, false);
                Ok(())
            }

            _ => todo!(),
        }
    }
}

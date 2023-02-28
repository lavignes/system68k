use super::Version;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Size {
    Byte,
    Word,
    Long,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Target {
    FromRegister,
    ToRegister,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EffectiveAddress {
    DataRegister(u8),
    AddressRegister(u8),
    Address(u8),
    AddressWithPostIncrement(u8),
    AddressWithPreDecrement(u8),
    AddressWithDisplacement(u8),
    AddressWithIndex(u8),
    PcWithDisplacement,
    PcWithIndex,
    AbsoluteShort,
    AbsoluteLong,
    Immediate, // TODO: Do we ever instanciate this ?
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Instruction {
    Illegal,
    OriToCcr,
    OriToSr,
    Ori(Size, EffectiveAddress),
    AndiToCcr,
    AndiToSr,
    Andi(Size, EffectiveAddress),
    Subi(Size, EffectiveAddress),
    Addi(Size, EffectiveAddress),
    EoriToCcr,
    EoriToSr,
    Eori(Size, EffectiveAddress),
    Cmpi(Size, EffectiveAddress),
    Btst(Option<u8>, EffectiveAddress),
    Bchg(Option<u8>, EffectiveAddress),
    Bclr(Option<u8>, EffectiveAddress),
    Bset(Option<u8>, EffectiveAddress),
    Movep(Size, Target, u8, u8),
}

#[derive(Debug)]
pub struct Decoder {
    version: Version,
    table: Vec<Instruction>,
}

impl Decoder {
    #[inline]
    pub fn new(version: Version) -> Self {
        Self {
            version,
            table: Self::init_table(version),
        }
    }

    #[inline]
    pub fn decode(&self, opcode: u16) -> Instruction {
        self.table[opcode as usize]
    }

    fn init_table(version: Version) -> Vec<Instruction> {
        let mut table = vec![Instruction::Illegal; 65536];
        for opcode in 0..table.len() {
            let opcode = opcode as u16;
            table[opcode as usize] = match (opcode & 0xF000) >> 12 {
                0x0 => Self::decode_0(version, opcode),
                0x1 => Self::decode_1(version, opcode),
                0x2 => Self::decode_2(version, opcode),
                0x3 => Self::decode_3(version, opcode),
                0x4 => Self::decode_4(version, opcode),
                0x5 => Self::decode_5(version, opcode),
                0x6 => Self::decode_6(version, opcode),
                0x7 => Self::decode_7(version, opcode),
                0x8 => Self::decode_8(version, opcode),
                0x9 => Self::decode_9(version, opcode),
                0xA => Self::decode_a(version, opcode),
                0xB => Self::decode_b(version, opcode),
                0xC => Self::decode_c(version, opcode),
                0xD => Self::decode_d(version, opcode),
                0xE => Self::decode_e(version, opcode),
                0xF => Self::decode_f(version, opcode),
                _ => unreachable!(),
            }
        }
        table
    }

    fn data_altering_address_mode(
        version: Version,
        mode: u8,
        register: u8,
    ) -> Option<EffectiveAddress> {
        match mode {
            0b000 => Some(EffectiveAddress::DataRegister(register)),
            0b001 => None,
            0b010 => Some(EffectiveAddress::Address(register)),
            0b011 => Some(EffectiveAddress::AddressWithPostIncrement(register)),
            0b100 => Some(EffectiveAddress::AddressWithPreDecrement(register)),
            0b101 => Some(EffectiveAddress::AddressWithDisplacement(register)),
            0b110 => Some(EffectiveAddress::AddressWithIndex(register)),
            0b111 => match register {
                0b000 => Some(EffectiveAddress::AbsoluteShort),
                0b001 => Some(EffectiveAddress::AbsoluteLong),
                _ => None,
            },
            _ => unreachable!(),
        }
    }

    fn bit_address_mode(version: Version, mode: u8, register: u8) -> Option<EffectiveAddress> {
        match mode {
            0b000 => Some(EffectiveAddress::DataRegister(register)),
            0b001 => None,
            0b010 => Some(EffectiveAddress::Address(register)),
            0b011 => Some(EffectiveAddress::AddressWithPostIncrement(register)),
            0b100 => Some(EffectiveAddress::AddressWithPreDecrement(register)),
            0b101 => Some(EffectiveAddress::AddressWithDisplacement(register)),
            0b110 => Some(EffectiveAddress::AddressWithIndex(register)),
            0b111 => match register {
                0b000 => Some(EffectiveAddress::AbsoluteShort),
                0b001 => Some(EffectiveAddress::AbsoluteLong),
                0b010 => Some(EffectiveAddress::PcWithDisplacement),
                0b011 => Some(EffectiveAddress::PcWithIndex),
                _ => None,
            },
            _ => unreachable!(),
        }
    }

    fn decode_0(version: Version, opcode: u16) -> Instruction {
        let bits0_2 = ((opcode & 0b0000_0000_0000_0111) >> 0) as u8;
        let bits3_5 = ((opcode & 0b0000_0000_0011_1000) >> 3) as u8;
        let bits6_7 = ((opcode & 0b0000_0000_1100_0000) >> 6) as u8;
        let bits8 = ((opcode & 0b0000_0001_0000_0000) >> 8) as u8;
        let bits9_11 = ((opcode & 0b0000_1110_0000_0000) >> 9) as u8;

        if bits8 == 0 {
            match bits9_11 {
                0b000 => {
                    if (bits0_2 == 4) && (bits3_5 == 7) {
                        return match bits6_7 {
                            0 => Instruction::OriToCcr,
                            1 => Instruction::OriToSr,
                            _ => Instruction::Illegal,
                        };
                    }

                    if let Some(ea) = Self::data_altering_address_mode(version, bits3_5, bits0_2) {
                        let size = match bits6_7 {
                            0 => Size::Byte,
                            1 => Size::Word,
                            2 => Size::Long,
                            _ => return Instruction::Illegal,
                        };
                        return Instruction::Ori(size, ea);
                    }
                }

                0b001 => {
                    if (bits0_2 == 4) && (bits3_5 == 7) {
                        return match bits6_7 {
                            0 => Instruction::AndiToCcr,
                            1 => Instruction::AndiToSr,
                            _ => Instruction::Illegal,
                        };
                    }

                    if let Some(ea) = Self::data_altering_address_mode(version, bits3_5, bits0_2) {
                        let size = match bits6_7 {
                            0 => Size::Byte,
                            1 => Size::Word,
                            2 => Size::Long,
                            _ => return Instruction::Illegal,
                        };
                        return Instruction::Andi(size, ea);
                    }
                }

                0b010 => {
                    if let Some(ea) = Self::data_altering_address_mode(version, bits3_5, bits0_2) {
                        let size = match bits6_7 {
                            0 => Size::Byte,
                            1 => Size::Word,
                            2 => Size::Long,
                            _ => return Instruction::Illegal,
                        };
                        return Instruction::Subi(size, ea);
                    }
                }

                0b011 => {
                    if let Some(ea) = Self::data_altering_address_mode(version, bits3_5, bits0_2) {
                        let size = match bits6_7 {
                            0 => Size::Byte,
                            1 => Size::Word,
                            2 => Size::Long,
                            _ => return Instruction::Illegal,
                        };
                        return Instruction::Addi(size, ea);
                    }
                }

                0b101 => {
                    if (bits0_2 == 4) && (bits3_5 == 7) {
                        return match bits6_7 {
                            0 => Instruction::EoriToCcr,
                            1 => Instruction::EoriToSr,
                            _ => Instruction::Illegal,
                        };
                    }

                    if let Some(ea) = Self::data_altering_address_mode(version, bits3_5, bits0_2) {
                        let size = match bits6_7 {
                            0 => Size::Byte,
                            1 => Size::Word,
                            2 => Size::Long,
                            _ => return Instruction::Illegal,
                        };
                        return Instruction::Eori(size, ea);
                    }
                }

                0b110 => {
                    if let Some(ea) = Self::data_altering_address_mode(version, bits3_5, bits0_2) {
                        let size = match bits6_7 {
                            0 => Size::Byte,
                            1 => Size::Word,
                            2 => Size::Long,
                            _ => return Instruction::Illegal,
                        };
                        return Instruction::Cmpi(size, ea);
                    }
                }

                0b100 => {
                    if let Some(ea) = Self::bit_address_mode(version, bits3_5, bits0_2) {
                        return match bits6_7 {
                            0 => Instruction::Btst(None, ea),
                            1 => Instruction::Bchg(None, ea),
                            2 => Instruction::Bclr(None, ea),
                            3 => Instruction::Bset(None, ea),
                            _ => Instruction::Illegal,
                        };
                    }
                }

                _ => return Instruction::Illegal,
            }
        }

        if bits3_5 != 1 {
            if let Some(ea) = Self::bit_address_mode(version, bits3_5, bits0_2) {
                let register = Some(bits9_11);
                return match bits6_7 {
                    0 => Instruction::Btst(register, ea),
                    1 => Instruction::Bchg(register, ea),
                    2 => Instruction::Bclr(register, ea),
                    3 => Instruction::Bset(register, ea),
                    _ => Instruction::Illegal,
                };
            }
        }

        let target = if (bits6_7 >> 1) == 0 {
            Target::FromRegister
        } else {
            Target::ToRegister
        };
        let size = if (bits6_7 & 1) == 0 {
            Size::Word
        } else {
            Size::Long
        };
        Instruction::Movep(size, target, bits9_11, bits0_2)
    }

    fn decode_1(version: Version, opcode: u16) -> Instruction {
        Instruction::Illegal
    }

    fn decode_2(version: Version, opcode: u16) -> Instruction {
        Instruction::Illegal
    }

    fn decode_3(version: Version, opcode: u16) -> Instruction {
        Instruction::Illegal
    }

    fn decode_4(version: Version, opcode: u16) -> Instruction {
        Instruction::Illegal
    }

    fn decode_5(version: Version, opcode: u16) -> Instruction {
        Instruction::Illegal
    }

    fn decode_6(version: Version, opcode: u16) -> Instruction {
        Instruction::Illegal
    }

    fn decode_7(version: Version, opcode: u16) -> Instruction {
        Instruction::Illegal
    }

    fn decode_8(version: Version, opcode: u16) -> Instruction {
        Instruction::Illegal
    }

    fn decode_9(version: Version, opcode: u16) -> Instruction {
        Instruction::Illegal
    }

    fn decode_a(version: Version, opcode: u16) -> Instruction {
        Instruction::Illegal
    }

    fn decode_b(version: Version, opcode: u16) -> Instruction {
        Instruction::Illegal
    }

    fn decode_c(version: Version, opcode: u16) -> Instruction {
        Instruction::Illegal
    }

    fn decode_d(version: Version, opcode: u16) -> Instruction {
        Instruction::Illegal
    }

    fn decode_e(version: Version, opcode: u16) -> Instruction {
        Instruction::Illegal
    }

    fn decode_f(version: Version, opcode: u16) -> Instruction {
        Instruction::Illegal
    }
}

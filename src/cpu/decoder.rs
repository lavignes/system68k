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

    Moveq(u8, u8),
}

lazy_static::lazy_static! {
    static ref TABLE_MC68000: Vec<Instruction> = init_table(Version::MC68000);
    static ref TABLE_MC68010: Vec<Instruction> = init_table(Version::MC68010);
}

#[derive(Debug)]
pub struct Decoder {
    table: &'static Vec<Instruction>,
}

impl Decoder {
    #[inline]
    pub fn new(version: Version) -> Self {
        match version {
            Version::MC68000 => Self {
                table: &TABLE_MC68000,
            },
            Version::MC68010 => Self {
                table: &TABLE_MC68010,
            },
        }
    }

    #[inline]
    pub fn decode(&self, opcode: u16) -> Instruction {
        self.table[opcode as usize]
    }
}
fn init_table(version: Version) -> Vec<Instruction> {
    let mut table = vec![Instruction::Illegal; 65536];
    for opcode in 0..table.len() {
        let opcode = opcode as u16;
        table[opcode as usize] = match (opcode & 0xF000) >> 12 {
            0x0 => decode_0(version, opcode),
            0x1 => decode_1(version, opcode),
            0x2 => decode_2(version, opcode),
            0x3 => decode_3(version, opcode),
            0x4 => decode_4(version, opcode),
            0x5 => decode_5(version, opcode),
            0x6 => decode_6(version, opcode),
            0x7 => decode_7(version, opcode),
            0x8 => decode_8(version, opcode),
            0x9 => decode_9(version, opcode),
            0xA => decode_a(version, opcode),
            0xB => decode_b(version, opcode),
            0xC => decode_c(version, opcode),
            0xD => decode_d(version, opcode),
            0xE => decode_e(version, opcode),
            0xF => decode_f(version, opcode),
            _ => unreachable!(),
        }
    }
    table
}

fn ea_type0(version: Version, mode: u8, register: u8) -> Option<EffectiveAddress> {
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

fn ea_type1(version: Version, mode: u8, register: u8) -> Option<EffectiveAddress> {
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
            0b100 => Some(EffectiveAddress::Immediate),
            _ => None,
        },
        _ => unreachable!(),
    }
}

fn ea_type2(version: Version, mode: u8, register: u8) -> Option<EffectiveAddress> {
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

                if let Some(ea) = ea_type0(version, bits3_5, bits0_2) {
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

                if let Some(ea) = ea_type0(version, bits3_5, bits0_2) {
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
                if let Some(ea) = ea_type0(version, bits3_5, bits0_2) {
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
                if let Some(ea) = ea_type0(version, bits3_5, bits0_2) {
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

                if let Some(ea) = ea_type0(version, bits3_5, bits0_2) {
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
                if let Some(ea) = ea_type0(version, bits3_5, bits0_2) {
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
                if let Some(ea) = ea_type2(version, bits3_5, bits0_2) {
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
        let register = Some(bits9_11);
        return match bits6_7 {
            // BTST Dn,<ea> has a weird edge-case where is allows immediate destination
            0 if let Some(ea) = ea_type1(version, bits3_5, bits0_2) => {
                Instruction::Btst(register, ea)
            }
            1 if let Some(ea) = ea_type2(version, bits3_5, bits0_2) => {
                Instruction::Bchg(register, ea)
            }
            2 if let Some(ea) = ea_type2(version, bits3_5, bits0_2) => {
                 Instruction::Bclr(register, ea)
            }
            3 if let Some(ea) = ea_type2(version, bits3_5, bits0_2) => {
                Instruction::Bset(register, ea)
            }
            _ => Instruction::Illegal
        };
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
    let bit8 = (opcode & 0b0000_0001_0000_0000) >> 8;
    let bits9_11 = (opcode & 0b0000_1110_0000_0000) >> 9;
    if bit8 == 1 {
        return Instruction::Illegal;
    }
    let data = (opcode & 0xFF) as u8;
    let register = bits9_11 as u8;
    Instruction::Moveq(data, register)
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

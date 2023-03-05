use super::*;
use crate::bus::TestBus;

#[rustfmt::skip]
const ROM1: &'static [u8] = &[
    0x00, 0x00, 0x10, 0x00, // stack $00001000
    0x00, 0x00, 0x04, 0x00, // pc    $00000400
];

#[test]
fn ori_to_ccr() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x00, 0x3C, 0x00, 0x07, // ORI #7, CCR
    ]);
    let mut cpu = Cpu::new(Version::MC68000);
    assert_eq!(Instruction::OriToCcr, cpu.decoder.decode(0x003C));

    bus.read8(0).unwrap();

    cpu.reset(&mut bus);

    cpu.set_sr(0x2700);
    cpu.step(&mut bus);

    assert_eq!(cpu.sr(), 0x2707);
}

#[test]
fn ori_to_sr() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x00, 0x7C, 0x07, 0x07, // ORI #$0707, SR
    ]);
    let mut cpu = Cpu::new(Version::MC68000);
    assert_eq!(Instruction::OriToSr, cpu.decoder.decode(0x007C));

    cpu.reset(&mut bus);

    cpu.set_sr(0x2000);
    cpu.step(&mut bus);

    assert_eq!(cpu.sr(), 0x2707);
}

#[test]
fn subi() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x04, 0x00, 0x00, 0x01, // SUBI.B #0,D0
    ]);
    let mut cpu = Cpu::new(Version::MC68000);
    assert_eq!(
        Instruction::Subi(Size::Byte, EffectiveAddress::DataRegister(0)),
        cpu.decoder.decode(0x0400)
    );

    cpu.reset(&mut bus);

    cpu.step(&mut bus);

    assert_eq!(cpu.data[0], 0x00FF);
    assert!(cpu.flag(StatusFlag::Carry));
    assert!(cpu.flag(StatusFlag::Extend));
    assert!(cpu.flag(StatusFlag::Negative));
    assert!(cpu.flag(StatusFlag::Overflow));
}

#[test]
fn btst() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x01, 0x3C, 0x00, 0x01, // BTST D0,#1
    ]);
    let mut cpu = Cpu::new(Version::MC68000);
    assert_eq!(
        Instruction::Btst(Some(0), EffectiveAddress::Immediate),
        cpu.decoder.decode(0x013C)
    );

    cpu.reset(&mut bus);

    cpu.step(&mut bus);

    assert!(!cpu.flag(StatusFlag::Zero));
}

#[test]
fn bchg() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x08, 0x40, 0x00, 0x01, // BCHG #1,D0
        0x08, 0x40, 0x00, 0x01, // BCHG #1,D0
    ]);
    let mut cpu = Cpu::new(Version::MC68000);
    assert_eq!(
        Instruction::Bchg(None, EffectiveAddress::DataRegister(0)),
        cpu.decoder.decode(0x0840)
    );

    cpu.reset(&mut bus);

    cpu.step(&mut bus);

    assert_eq!(cpu.data[0], 2);
    assert!(cpu.flag(StatusFlag::Zero));

    cpu.step(&mut bus);

    assert_eq!(cpu.data[0], 0);
    assert!(!cpu.flag(StatusFlag::Zero));
}

#[test]
fn bclr() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x08, 0x80, 0x00, 0x01, // BCLR #1,D0
    ]);
    let mut cpu = Cpu::new(Version::MC68000);
    assert_eq!(
        Instruction::Bclr(None, EffectiveAddress::DataRegister(0)),
        cpu.decoder.decode(0x0880)
    );

    cpu.reset(&mut bus);

    cpu.step(&mut bus);

    assert!(cpu.flag(StatusFlag::Zero));
}

#[test]
fn bset() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x08, 0xC0, 0x00, 0x01, // BSET #1,D0
    ]);
    let mut cpu = Cpu::new(Version::MC68000);
    assert_eq!(
        Instruction::Bset(None, EffectiveAddress::DataRegister(0)),
        cpu.decoder.decode(0x08C0)
    );

    cpu.reset(&mut bus);

    cpu.step(&mut bus);

    assert_eq!(cpu.data[0], 2);
    assert!(cpu.flag(StatusFlag::Zero));
}

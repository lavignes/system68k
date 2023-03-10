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
        0x00, 0x3C, 0x00, 0x07, // ORI #7,CCR
    ]);
    let mut cpu = Cpu::new();
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
        0x00, 0x7C, 0x07, 0x07, // ORI #$0707,SR
    ]);
    let mut cpu = Cpu::new();
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
    let mut cpu = Cpu::new();
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
    let mut cpu = Cpu::new();
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
    let mut cpu = Cpu::new();
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
    let mut cpu = Cpu::new();
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
    let mut cpu = Cpu::new();
    assert_eq!(
        Instruction::Bset(None, EffectiveAddress::DataRegister(0)),
        cpu.decoder.decode(0x08C0)
    );

    cpu.reset(&mut bus);

    cpu.step(&mut bus);

    assert_eq!(cpu.data[0], 2);
    assert!(cpu.flag(StatusFlag::Zero));
}

#[test]
fn movea() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x30, 0x40, // MOVEA.W D0,A0
    ]);
    let mut cpu = Cpu::new();
    assert_eq!(
        Instruction::Movea(Size::Word, EffectiveAddress::DataRegister(0), 0),
        cpu.decoder.decode(0x3040)
    );

    cpu.reset(&mut bus);
    cpu.data[0] = 0x12345678;
    cpu.addr[0] = 0xFFFF0000;

    cpu.step(&mut bus);

    assert_eq!(cpu.addr[0], 0xFFFF5678);
}

#[test]
fn r#move() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x12, 0x00, // MOVE.B D0,D1
    ]);
    let mut cpu = Cpu::new();
    assert_eq!(
        Instruction::Move(
            Size::Byte,
            EffectiveAddress::DataRegister(0),
            EffectiveAddress::DataRegister(1)
        ),
        cpu.decoder.decode(0x1200)
    );

    cpu.reset(&mut bus);
    cpu.data[0] = 0x12345678;

    cpu.step(&mut bus);

    assert_eq!(cpu.data[1], 0x00000078);
}

#[test]
fn move_from_sr() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x40, 0xC0, // MOVE SR,D0
    ]);
    let mut cpu = Cpu::new();
    assert_eq!(
        Instruction::MoveFromSr(EffectiveAddress::DataRegister(0)),
        cpu.decoder.decode(0x40C0)
    );

    cpu.reset(&mut bus);
    cpu.set_sr(0x2700);

    cpu.step(&mut bus);

    assert_eq!(cpu.data[0], 0x2700);
}

#[test]
fn move_to_ccr() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x44, 0xC0, // MOVE D0,CCR
    ]);
    let mut cpu = Cpu::new();
    assert_eq!(
        Instruction::MoveToCcr(EffectiveAddress::DataRegister(0)),
        cpu.decoder.decode(0x44C0)
    );

    cpu.reset(&mut bus);
    cpu.data[0] = 0x1F;

    cpu.step(&mut bus);

    assert_eq!(cpu.sr, 0x271F);
}

#[test]
fn move_to_sr() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x46, 0xC0, // MOVE D0,SR
    ]);
    let mut cpu = Cpu::new();
    assert_eq!(
        Instruction::MoveToSr(EffectiveAddress::DataRegister(0)),
        cpu.decoder.decode(0x46C0)
    );

    cpu.reset(&mut bus);
    cpu.data[0] = 0xA71F;

    cpu.step(&mut bus);

    assert_eq!(cpu.sr, 0xA71F);
}

#[test]
fn negx() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x40, 0x80, // NEGX.L D0
    ]);
    let mut cpu = Cpu::new();
    assert_eq!(
        Instruction::Negx(Size::Long, EffectiveAddress::DataRegister(0)),
        cpu.decoder.decode(0x4080)
    );

    cpu.reset(&mut bus);
    cpu.data[0] = 1;

    cpu.step(&mut bus);

    assert_eq!(cpu.data[0], 0xFFFFFFFF);
    assert!(cpu.flag(StatusFlag::Carry));
    assert!(!cpu.flag(StatusFlag::Zero));
    assert!(cpu.flag(StatusFlag::Overflow));
    assert!(cpu.flag(StatusFlag::Negative));
    assert!(cpu.flag(StatusFlag::Extend));
}

#[test]
fn clr() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x42, 0x40, // CLR.W D0
    ]);
    let mut cpu = Cpu::new();
    assert_eq!(
        Instruction::Clr(Size::Word, EffectiveAddress::DataRegister(0)),
        cpu.decoder.decode(0x4240)
    );

    cpu.reset(&mut bus);
    cpu.data[0] = 0xFFFFFFFF;
    cpu.set_flag(StatusFlag::Extend, true);

    cpu.step(&mut bus);

    assert_eq!(cpu.data[0], 0xFFFF0000);
    assert!(!cpu.flag(StatusFlag::Carry));
    assert!(cpu.flag(StatusFlag::Zero));
    assert!(!cpu.flag(StatusFlag::Overflow));
    assert!(!cpu.flag(StatusFlag::Negative));
    assert!(cpu.flag(StatusFlag::Extend));
}

#[test]
fn neg() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x44, 0x00, // NEG.B D0
    ]);
    let mut cpu = Cpu::new();
    assert_eq!(
        Instruction::Neg(Size::Byte, EffectiveAddress::DataRegister(0)),
        cpu.decoder.decode(0x4400)
    );

    cpu.reset(&mut bus);
    cpu.data[0] = 1;

    cpu.step(&mut bus);

    assert_eq!(cpu.data[0], 0x000000FF);
    assert!(cpu.flag(StatusFlag::Carry));
    assert!(!cpu.flag(StatusFlag::Zero));
    assert!(cpu.flag(StatusFlag::Overflow));
    assert!(cpu.flag(StatusFlag::Negative));
    assert!(cpu.flag(StatusFlag::Extend));
}

#[test]
fn not() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x46, 0x40, // NOT.W D0
    ]);
    let mut cpu = Cpu::new();
    assert_eq!(
        Instruction::Not(Size::Word, EffectiveAddress::DataRegister(0)),
        cpu.decoder.decode(0x4640)
    );

    cpu.reset(&mut bus);
    cpu.data[0] = 0x00FF;

    cpu.step(&mut bus);

    assert_eq!(cpu.data[0], 0x0000FF00);
    assert!(!cpu.flag(StatusFlag::Zero));
    assert!(cpu.flag(StatusFlag::Negative));
    assert!(!cpu.flag(StatusFlag::Overflow));
    assert!(!cpu.flag(StatusFlag::Carry));
}

#[test]
fn ext() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x48, 0x80, // EXT.W D0
    ]);
    let mut cpu = Cpu::new();
    assert_eq!(Instruction::Ext(Size::Word, 0), cpu.decoder.decode(0x4880));

    cpu.reset(&mut bus);
    cpu.data[0] = 0x80;

    cpu.step(&mut bus);

    assert_eq!(cpu.data[0], 0x0000FF80);
    assert!(!cpu.flag(StatusFlag::Zero));
    assert!(cpu.flag(StatusFlag::Negative));
    assert!(!cpu.flag(StatusFlag::Overflow));
    assert!(!cpu.flag(StatusFlag::Carry));
}

#[test]
fn swap() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x48, 0x40, // SWAP D0
    ]);
    let mut cpu = Cpu::new();
    assert_eq!(Instruction::Swap(0), cpu.decoder.decode(0x4840));

    cpu.reset(&mut bus);
    cpu.data[0] = 0x12345678;

    cpu.step(&mut bus);

    assert_eq!(cpu.data[0], 0x56781234);
    assert!(!cpu.flag(StatusFlag::Zero));
    assert!(!cpu.flag(StatusFlag::Negative));
    assert!(!cpu.flag(StatusFlag::Overflow));
    assert!(!cpu.flag(StatusFlag::Carry));
}

#[test]
fn pea() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x48, 0x78, 0x04, 0x00 // PEA ($0400).W
    ]);
    let mut cpu = Cpu::new();
    assert_eq!(
        Instruction::Pea(EffectiveAddress::AbsoluteShort),
        cpu.decoder.decode(0x4878)
    );

    cpu.reset(&mut bus);

    cpu.step(&mut bus);

    assert_eq!(cpu.ssp, 0x0FFC);
    assert_eq!(bus.mem()[0x00000FFC], 0x48);
    assert_eq!(bus.mem()[0x00000FFD], 0x78);
    assert_eq!(bus.mem()[0x00000FFE], 0x04);
    assert_eq!(bus.mem()[0x00000FFF], 0x00);
}

#[test]
fn tas() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x4A, 0xC0, // TAS D0
    ]);
    let mut cpu = Cpu::new();
    assert_eq!(
        Instruction::Tas(EffectiveAddress::DataRegister(0)),
        cpu.decoder.decode(0x4AC0)
    );

    cpu.reset(&mut bus);
    cpu.data[0] = 0x80;

    cpu.step(&mut bus);

    assert!(!cpu.flag(StatusFlag::Zero));
    assert!(cpu.flag(StatusFlag::Negative));
    assert_eq!(cpu.data[0], 0x80);
}

#[test]
fn tst() {
    #[rustfmt::skip]
    let mut bus = TestBus::new(ROM1, 0x0400, 0x1000, &[
        0x4A, 0x07, // TST.B D7
    ]);
    let mut cpu = Cpu::new();
    assert_eq!(
        Instruction::Tst(Size::Byte, EffectiveAddress::DataRegister(7)),
        cpu.decoder.decode(0x4A07)
    );

    cpu.reset(&mut bus);
    cpu.data[7] = 0x80;

    cpu.step(&mut bus);

    assert!(!cpu.flag(StatusFlag::Zero));
    assert!(cpu.flag(StatusFlag::Negative));
}

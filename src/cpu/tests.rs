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

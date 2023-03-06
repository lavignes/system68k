use std::{
    collections::HashSet,
    io::{Cursor, Read, Write},
    num::NonZeroUsize,
};

use gdbstub::{
    arch::{Arch, BreakpointKind, RegId, Registers, SingleStepGdbBehavior},
    common::Signal,
    target::{
        ext::{
            base::{
                single_register_access::{SingleRegisterAccess, SingleRegisterAccessOps},
                singlethread::{SingleThreadBase, SingleThreadResume, SingleThreadResumeOps},
                BaseOps,
            },
            breakpoints::{Breakpoints, BreakpointsOps, SwBreakpoint, SwBreakpointOps},
        },
        Target, TargetResult,
    },
};
use system68k::{bus::Bus, cpu::Cpu, sys::System};

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub struct MC68kCoreRegs {
    data: [u32; 8],
    addr: [u32; 8],
    sr: u32,
    pc: u32,
}

impl Registers for MC68kCoreRegs {
    type ProgramCounter = u32;

    #[inline]
    fn pc(&self) -> Self::ProgramCounter {
        self.pc
    }

    #[inline]
    fn gdb_serialize(&self, mut write_byte: impl FnMut(Option<u8>)) {
        for register in self.data {
            for byte in register.to_le_bytes() {
                write_byte(Some(byte));
            }
        }

        for register in self.addr {
            for byte in register.to_le_bytes() {
                write_byte(Some(byte));
            }
        }

        for byte in self.sr.to_le_bytes() {
            write_byte(Some(byte));
        }

        for byte in self.pc.to_le_bytes() {
            write_byte(Some(byte));
        }
    }

    #[inline]
    fn gdb_deserialize(&mut self, bytes: &[u8]) -> Result<(), ()> {
        let mut reader = Cursor::new(bytes);

        for register in self.data.iter_mut() {
            let mut bytes = [0; 4];
            reader.read_exact(&mut bytes).map_err(|_| ())?;
            *register = u32::from_le_bytes(bytes);
        }

        for register in self.addr.iter_mut() {
            let mut bytes = [0; 4];
            reader.read_exact(&mut bytes).map_err(|_| ())?;
            *register = u32::from_le_bytes(bytes);
        }

        {
            let mut bytes = [0; 4];
            reader.read_exact(&mut bytes).map_err(|_| ())?;
            self.sr = u32::from_le_bytes(bytes);
        }

        {
            let mut bytes = [0; 4];
            reader.read_exact(&mut bytes).map_err(|_| ())?;
            self.pc = u32::from_le_bytes(bytes);
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum MC68kRegId {
    Data(usize),
    Addr(usize),
    Sr,
    Pc,
}

impl RegId for MC68kRegId {
    #[inline]
    fn from_raw_id(id: usize) -> Option<(Self, Option<NonZeroUsize>)> {
        let register = match id {
            0..=7 => Self::Data(id),
            8..=15 => Self::Addr(id - 8),
            16 => Self::Sr,
            17 => Self::Pc,
            _ => return None,
        };
        Some((register, Some(NonZeroUsize::new(4)?)))
    }
}

#[derive(Debug)]
pub struct MC68kBreakpointKind;

impl BreakpointKind for MC68kBreakpointKind {
    #[inline]
    fn from_usize(kind: usize) -> Option<Self> {
        Some(Self)
    }
}

pub struct MC68k;

impl Arch for MC68k {
    type Usize = u32;
    type Registers = MC68kCoreRegs;
    type RegId = MC68kRegId;
    type BreakpointKind = MC68kBreakpointKind;

    #[inline]
    fn target_description_xml() -> Option<&'static str> {
        None
    }

    #[inline]
    fn single_step_gdb_behavior() -> SingleStepGdbBehavior {
        SingleStepGdbBehavior::Optional
    }
}

pub struct SystemTarget {
    sys: System,
    breakpoints: HashSet<u32>,
}

impl SystemTarget {
    #[inline]
    pub fn new(sys: System) -> Self {
        Self {
            sys,
            breakpoints: HashSet::new(),
        }
    }

    #[inline]
    pub fn cpu(&self) -> &Cpu {
        &self.sys.cpu()
    }

    #[inline]
    pub fn step(&mut self) -> bool {
        let Self { sys, breakpoints } = self;

        sys.step();
        let pc = sys.cpu().pc();

        if breakpoints.contains(&pc) {
            return true;
        }
        false
    }
}

impl Target for SystemTarget {
    type Arch = MC68k;
    type Error = &'static str;

    #[inline]
    fn base_ops(&mut self) -> BaseOps<'_, Self::Arch, Self::Error> {
        BaseOps::SingleThread(self)
    }

    #[inline]
    fn support_breakpoints(&mut self) -> Option<BreakpointsOps<'_, Self>> {
        Some(self)
    }
}

impl SingleThreadBase for SystemTarget {
    #[inline]
    fn read_registers(
        &mut self,
        regs: &mut <Self::Arch as Arch>::Registers,
    ) -> TargetResult<(), Self> {
        let cpu = self.sys.cpu();
        for register in 0usize..=7 {
            regs.data[register] = cpu.data(register);
            regs.addr[register] = cpu.addr(register);
        }
        regs.sr = cpu.sr() as u32;
        regs.pc = cpu.pc();
        Ok(())
    }

    #[inline]
    fn write_registers(
        &mut self,
        regs: &<Self::Arch as Arch>::Registers,
    ) -> TargetResult<(), Self> {
        let cpu = self.sys.cpu_mut();
        for register in 0usize..=7 {
            cpu.set_data(register, regs.data[register]);
            cpu.set_addr(register, regs.addr[register]);
        }
        cpu.set_sr(regs.sr as u16);
        cpu.set_pc(regs.pc);
        Ok(())
    }

    #[inline]
    fn read_addrs(
        &mut self,
        start_addr: <Self::Arch as Arch>::Usize,
        data: &mut [u8],
    ) -> TargetResult<(), Self> {
        for i in (start_addr as usize)..data.len() {
            data[i] = self.sys.read8(i as u32).map_err(|_| ())?;
        }
        Ok(())
    }

    #[inline]
    fn write_addrs(
        &mut self,
        start_addr: <Self::Arch as Arch>::Usize,
        data: &[u8],
    ) -> TargetResult<(), Self> {
        for i in (start_addr as usize)..data.len() {
            self.sys.write8(i as u32, data[i]).map_err(|_| ())?;
        }
        Ok(())
    }

    #[inline]
    fn support_single_register_access(&mut self) -> Option<SingleRegisterAccessOps<'_, (), Self>> {
        Some(self)
    }

    #[inline]
    fn support_resume(&mut self) -> Option<SingleThreadResumeOps<'_, Self>> {
        Some(self)
    }
}

impl SingleRegisterAccess<()> for SystemTarget {
    #[inline]
    fn read_register(
        &mut self,
        tid: (),
        reg_id: <Self::Arch as Arch>::RegId,
        mut buf: &mut [u8],
    ) -> TargetResult<usize, Self> {
        let cpu = self.sys.cpu();
        let value = match reg_id {
            MC68kRegId::Data(register) => cpu.data(register),
            MC68kRegId::Addr(register) => cpu.addr(register),
            MC68kRegId::Sr => cpu.sr() as u32,
            MC68kRegId::Pc => cpu.pc(),
        };
        buf.write_all(&value.to_le_bytes()).map_err(|_| ())?;
        Ok(4)
    }

    #[inline]
    fn write_register(
        &mut self,
        tid: (),
        reg_id: <Self::Arch as Arch>::RegId,
        val: &[u8],
    ) -> TargetResult<(), Self> {
        let cpu = self.sys.cpu_mut();
        let value = u32::from_le_bytes(val[0..4].try_into().map_err(|_| ())?);
        match reg_id {
            MC68kRegId::Data(register) => cpu.set_data(register, value),
            MC68kRegId::Addr(register) => cpu.set_addr(register, value),
            MC68kRegId::Sr => cpu.set_sr(value as u16),
            MC68kRegId::Pc => cpu.set_pc(value),
        };
        Ok(())
    }
}

impl Breakpoints for SystemTarget {
    #[inline]
    fn support_sw_breakpoint(&mut self) -> Option<SwBreakpointOps<'_, Self>> {
        Some(self)
    }
}

impl SwBreakpoint for SystemTarget {
    #[inline]
    fn add_sw_breakpoint(
        &mut self,
        addr: <Self::Arch as Arch>::Usize,
        kind: <Self::Arch as Arch>::BreakpointKind,
    ) -> TargetResult<bool, Self> {
        Ok(self.breakpoints.insert(addr))
    }

    #[inline]
    fn remove_sw_breakpoint(
        &mut self,
        addr: <Self::Arch as Arch>::Usize,
        kind: <Self::Arch as Arch>::BreakpointKind,
    ) -> TargetResult<bool, Self> {
        Ok(self.breakpoints.remove(&addr))
    }
}

impl SingleThreadResume for SystemTarget {
    fn resume(&mut self, signal: Option<Signal>) -> Result<(), Self::Error> {
        if signal.is_some() {
            return Err("no support for resuming from a signal");
        }
        Ok(())
    }
}

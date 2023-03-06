use crate::{
    bus::{self, Bus},
    cpu::{Cpu, Version},
};

pub struct System {
    cpu: Cpu,
    rom: Vec<u8>,
    ram: Vec<u8>,
}

impl System {
    #[inline]
    pub fn new<Rom: AsRef<[u8]>>(rom: Rom) -> Self {
        Self {
            cpu: Cpu::new(Version::MC68000),
            rom: rom.as_ref().to_vec(),
            ram: vec![0; 0x01000000],
        }
    }

    #[inline]
    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    #[inline]
    pub fn cpu_mut(&mut self) -> &mut Cpu {
        &mut self.cpu
    }

    #[inline]
    pub fn reset(&mut self) {
        let Self { cpu, rom, ram } = self;
        let mut view = CpuView { rom, ram };
        cpu.reset(&mut view);
    }

    #[inline]
    pub fn step(&mut self) {
        let Self { cpu, rom, ram } = self;
        let mut view = CpuView { rom, ram };
        cpu.step(&mut view);
    }
}

impl Bus for System {
    #[inline]
    fn read8(&self, addr: u32) -> Result<u8, bus::Error> {
        let addr = addr as usize;
        if addr < 0x00010000 {
            return Ok(self.rom[addr]);
        }

        if addr < 0x01000000 {
            return Ok(self.ram[addr]);
        }

        Err(bus::Error::BusError)
    }

    #[inline]
    fn read16(&self, addr: u32) -> Result<u16, bus::Error> {
        let addr = addr as usize;
        if addr < 0x00010000 {
            return Ok(u16::from_be_bytes([self.rom[addr + 0], self.rom[addr + 1]]));
        }
        if addr < 0x01000000 {
            return Ok(u16::from_be_bytes([self.ram[addr + 0], self.ram[addr + 1]]));
        }

        Err(bus::Error::BusError)
    }

    #[inline]
    fn read32(&self, addr: u32) -> Result<u32, bus::Error> {
        let addr = addr as usize;
        if addr < 0x00010000 {
            return Ok(u32::from_be_bytes([
                self.rom[addr + 0],
                self.rom[addr + 1],
                self.rom[addr + 2],
                self.rom[addr + 3],
            ]));
        }

        if addr < 0x01000000 {
            return Ok(u32::from_be_bytes([
                self.ram[addr + 0],
                self.ram[addr + 1],
                self.ram[addr + 2],
                self.ram[addr + 3],
            ]));
        }

        Err(bus::Error::BusError)
    }

    #[inline]
    fn write8(&mut self, addr: u32, value: u8) -> Result<(), bus::Error> {
        let addr = addr as usize;
        if addr < 0x00010000 {
            return Err(bus::Error::BusError);
        }

        if addr < 0x01000000 {
            self.ram[addr] = value;
            return Ok(());
        }

        Err(bus::Error::BusError)
    }

    #[inline]
    fn write16(&mut self, addr: u32, value: u16) -> Result<(), bus::Error> {
        let addr = addr as usize;
        if addr < 0x00010000 {
            return Err(bus::Error::BusError);
        }

        if addr < 0x01000000 {
            let bytes = value.to_be_bytes();
            self.ram[addr + 0] = bytes[0];
            self.ram[addr + 1] = bytes[1];
            return Ok(());
        }

        Err(bus::Error::BusError)
    }

    #[inline]
    fn write32(&mut self, addr: u32, value: u32) -> Result<(), bus::Error> {
        let addr = addr as usize;
        if addr < 0x00010000 {
            return Err(bus::Error::BusError);
        }

        if addr < 0x01000000 {
            let bytes = value.to_be_bytes();
            self.ram[addr + 0] = bytes[0];
            self.ram[addr + 1] = bytes[1];
            self.ram[addr + 2] = bytes[2];
            self.ram[addr + 3] = bytes[3];
            return Ok(());
        }

        Err(bus::Error::BusError)
    }
}

pub struct CpuView<'a> {
    rom: &'a mut Vec<u8>,
    ram: &'a mut Vec<u8>,
}

impl<'a> Bus for CpuView<'a> {
    #[inline]
    fn read8(&self, addr: u32) -> Result<u8, bus::Error> {
        let addr = addr as usize;
        if addr < 0x00010000 {
            return Ok(self.rom[addr]);
        }

        if addr < 0x01000000 {
            return Ok(self.ram[addr]);
        }

        Err(bus::Error::BusError)
    }

    #[inline]
    fn read16(&self, addr: u32) -> Result<u16, bus::Error> {
        let addr = addr as usize;
        if addr < 0x00010000 {
            return Ok(u16::from_be_bytes([self.rom[addr + 0], self.rom[addr + 1]]));
        }
        if addr < 0x01000000 {
            return Ok(u16::from_be_bytes([self.ram[addr + 0], self.ram[addr + 1]]));
        }

        Err(bus::Error::BusError)
    }

    #[inline]
    fn read32(&self, addr: u32) -> Result<u32, bus::Error> {
        let addr = addr as usize;
        if addr < 0x00010000 {
            return Ok(u32::from_be_bytes([
                self.rom[addr + 0],
                self.rom[addr + 1],
                self.rom[addr + 2],
                self.rom[addr + 3],
            ]));
        }

        if addr < 0x01000000 {
            return Ok(u32::from_be_bytes([
                self.ram[addr + 0],
                self.ram[addr + 1],
                self.ram[addr + 2],
                self.ram[addr + 3],
            ]));
        }

        Err(bus::Error::BusError)
    }

    #[inline]
    fn write8(&mut self, addr: u32, value: u8) -> Result<(), bus::Error> {
        let addr = addr as usize;
        if addr < 0x00010000 {
            return Err(bus::Error::BusError);
        }

        if addr < 0x01000000 {
            self.ram[addr] = value;
            return Ok(());
        }

        Err(bus::Error::BusError)
    }

    #[inline]
    fn write16(&mut self, addr: u32, value: u16) -> Result<(), bus::Error> {
        let addr = addr as usize;
        if addr < 0x00010000 {
            return Err(bus::Error::BusError);
        }

        if addr < 0x01000000 {
            let bytes = value.to_be_bytes();
            self.ram[addr + 0] = bytes[0];
            self.ram[addr + 1] = bytes[1];
            return Ok(());
        }

        Err(bus::Error::BusError)
    }

    #[inline]
    fn write32(&mut self, addr: u32, value: u32) -> Result<(), bus::Error> {
        let addr = addr as usize;
        if addr < 0x00010000 {
            return Err(bus::Error::BusError);
        }

        if addr < 0x01000000 {
            let bytes = value.to_be_bytes();
            self.ram[addr + 0] = bytes[0];
            self.ram[addr + 1] = bytes[1];
            self.ram[addr + 2] = bytes[2];
            self.ram[addr + 3] = bytes[3];
            return Ok(());
        }

        Err(bus::Error::BusError)
    }
}

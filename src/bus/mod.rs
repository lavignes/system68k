#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("bus error")]
    BusError,
}

pub trait Bus {
    fn read8(&self, addr: u32) -> Result<u8, Error>;

    fn read16(&self, addr: u32) -> Result<u16, Error>;

    fn read32(&self, addr: u32) -> Result<u32, Error>;

    fn write8(&mut self, addr: u32, value: u8) -> Result<(), Error>;

    fn write16(&mut self, addr: u32, value: u16) -> Result<(), Error>;

    fn write32(&mut self, addr: u32, value: u32) -> Result<(), Error>;
}

pub struct TestBus {
    mem: Vec<u8>,
}

impl TestBus {
    #[inline]
    pub fn new(rom: &[u8], ram_base: u32, ram_pad: u32, ram: &[u8]) -> Self {
        let mut mem = rom.to_vec();
        mem.resize(ram_base as usize, 0x00);
        mem.extend_from_slice(ram);
        mem.resize(ram_pad as usize, 0x00);
        Self { mem }
    }

    #[inline]
    fn mem(&self) -> &[u8] {
        &self.mem
    }
}

impl Bus for TestBus {
    #[inline]
    fn read8(&self, addr: u32) -> Result<u8, Error> {
        let addr = addr as usize;
        Ok(self.mem[addr])
    }

    #[inline]
    fn read16(&self, addr: u32) -> Result<u16, Error> {
        let addr = addr as usize;
        Ok(u16::from_be_bytes([self.mem[addr], self.mem[addr + 1]]))
    }

    #[inline]
    fn read32(&self, addr: u32) -> Result<u32, Error> {
        let addr = addr as usize;
        Ok(u32::from_be_bytes([
            self.mem[addr + 0],
            self.mem[addr + 1],
            self.mem[addr + 2],
            self.mem[addr + 3],
        ]))
    }

    #[inline]
    fn write8(&mut self, addr: u32, value: u8) -> Result<(), Error> {
        let addr = addr as usize;
        self.mem[addr] = value;
        Ok(())
    }

    #[inline]
    fn write16(&mut self, addr: u32, value: u16) -> Result<(), Error> {
        let addr = addr as usize;
        let bytes = value.to_be_bytes();
        self.mem[addr + 0] = bytes[0];
        self.mem[addr + 1] = bytes[1];
        Ok(())
    }

    #[inline]
    fn write32(&mut self, addr: u32, value: u32) -> Result<(), Error> {
        let addr = addr as usize;
        let bytes = value.to_be_bytes();
        self.mem[addr + 0] = bytes[0];
        self.mem[addr + 1] = bytes[1];
        self.mem[addr + 2] = bytes[2];
        self.mem[addr + 3] = bytes[3];
        Ok(())
    }
}

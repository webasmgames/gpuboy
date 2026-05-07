use crate::cartridge::Cartridge;

pub struct Bus {
    cartridge: Cartridge,
    wram: [u8; 0x2000],
    hram: [u8; 0x7F],
}

impl Bus {
    pub fn new(cartridge: Cartridge) -> Self {
        Bus {
            cartridge,
            wram: [0; 0x2000],
            hram: [0; 0x7F],
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.cartridge.read(addr),
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize],
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF => {}
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = val,
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize] = val,
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = val,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::Cartridge;

    fn minimal_rom(mbc_type: u8) -> Vec<u8> {
        let mut rom = vec![0u8; 0x014A];
        for (i, &b) in b"TEST".iter().enumerate() {
            rom[0x0134 + i] = b;
        }
        rom[0x0147] = mbc_type;
        rom
    }

    fn test_bus() -> Bus {
        Bus::new(Cartridge::load(minimal_rom(0x00)).unwrap())
    }

    #[test]
    fn wram_roundtrip() {
        let mut bus = test_bus();
        bus.write(0xC000, 0x42);
        assert_eq!(bus.read(0xC000), 0x42);
    }

    #[test]
    fn hram_roundtrip() {
        let mut bus = test_bus();
        bus.write(0xFF80, 0xAB);
        assert_eq!(bus.read(0xFF80), 0xAB);
    }

    #[test]
    fn echo_ram_mirrors_wram() {
        let mut bus = test_bus();
        bus.write(0xC000, 0x55);
        assert_eq!(bus.read(0xE000), 0x55);
    }

    #[test]
    fn unmapped_reads_ff() {
        let bus = test_bus();
        assert_eq!(bus.read(0xFF00), 0xFF);
        assert_eq!(bus.read(0xFFFF), 0xFF);
    }

    #[test]
    fn rom_write_silent() {
        let mut bus = test_bus();
        let original = bus.read(0x0000);
        bus.write(0x0000, 0xAA);
        assert_eq!(bus.read(0x0000), original);
    }
}

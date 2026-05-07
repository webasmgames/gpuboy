#[derive(Debug)]
pub struct CartridgeHeader {
    pub title: String,
    pub cartridge_type: u8,
    pub rom_size: u8,
    pub ram_size: u8,
}

impl CartridgeHeader {
    pub fn parse(rom: &[u8]) -> Result<Self, String> {
        if rom.len() < 0x014A {
            return Err("ROM too small".into());
        }
        let title = rom[0x0134..0x0144]
            .iter()
            .copied()
            .take_while(|&b| b != 0 && (b.is_ascii_graphic() || b == b' '))
            .map(|b| b as char)
            .collect();
        Ok(CartridgeHeader {
            title,
            cartridge_type: rom[0x0147],
            rom_size: rom[0x0148],
            ram_size: rom[0x0149],
        })
    }
}

#[derive(Debug)]
pub struct Cartridge {
    pub header: CartridgeHeader,
    rom: Vec<u8>,
}

impl Cartridge {
    pub fn load(rom: Vec<u8>) -> Result<Self, String> {
        let header = CartridgeHeader::parse(&rom)?;
        if header.cartridge_type != 0x00 {
            return Err(format!("unsupported MBC: 0x{:02X}", header.cartridge_type));
        }
        Ok(Cartridge { header, rom })
    }

    pub fn read(&self, addr: u16) -> u8 {
        self.rom.get(addr as usize).copied().unwrap_or(0xFF)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_rom(mbc_type: u8) -> Vec<u8> {
        let mut rom = vec![0u8; 0x014A];
        for (i, &b) in b"TEST".iter().enumerate() {
            rom[0x0134 + i] = b;
        }
        rom[0x0147] = mbc_type;
        rom
    }

    fn real_rom_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/roms/blargg/cpu_instrs/cpu_instrs.gb")
    }

    #[test]
    fn parse_header_real_rom() {
        let rom = std::fs::read(real_rom_path()).expect("run scripts/download-test-roms.sh");
        let header = CartridgeHeader::parse(&rom).unwrap();
        assert_eq!(header.title, "CPU_INSTRS");
        assert_eq!(header.cartridge_type, 0x01);
        assert_eq!(header.rom_size, 0x01);
        assert_eq!(header.ram_size, 0x00);
    }

    #[test]
    fn load_unsupported_mbc_real_rom() {
        let rom = std::fs::read(real_rom_path()).expect("run scripts/download-test-roms.sh");
        let err = Cartridge::load(rom).unwrap_err();
        assert!(err.contains("unsupported MBC"), "got: {err}");
    }

    #[test]
    fn load_too_small() {
        let err = Cartridge::load(vec![0u8; 10]).unwrap_err();
        assert!(err.contains("ROM too small"), "got: {err}");
    }

    #[test]
    fn load_flat_rom() {
        let rom = minimal_rom(0x00);
        let cart = Cartridge::load(rom).unwrap();
        assert_eq!(cart.header.cartridge_type, 0x00);
    }

    #[test]
    fn read_out_of_bounds() {
        let rom = minimal_rom(0x00);
        let cart = Cartridge::load(rom).unwrap();
        assert_eq!(cart.read(0x7FFF), 0xFF);
    }
}

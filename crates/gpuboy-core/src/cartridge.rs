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
pub enum Mbc {
    None,
    Mbc1 {
        rom_bank: u8,
        ram_bank: u8,
        mode: bool,
        ram_enabled: bool,
    },
    Mbc3 {
        rom_bank: u8,
        ram_bank: u8,
        ram_enabled: bool,
    },
    Mbc5 {
        rom_bank_lo: u8,
        rom_bank_hi: u8,
        ram_bank: u8,
        ram_enabled: bool,
    },
}

#[derive(Debug)]
pub struct Cartridge {
    pub header: CartridgeHeader,
    rom: Vec<u8>,
    sram: Vec<u8>,
    mbc: Mbc,
}

fn sram_size(ram_size_byte: u8) -> usize {
    match ram_size_byte {
        0x02 => 8192,
        0x03 => 32768,
        0x04 => 131072,
        0x05 => 65536,
        _ => 0,
    }
}

impl Cartridge {
    pub fn load(rom: Vec<u8>) -> Result<Self, String> {
        let header = CartridgeHeader::parse(&rom)?;
        let sram = vec![0u8; sram_size(header.ram_size)];
        let mbc = match header.cartridge_type {
            0x00 => Mbc::None,
            0x01..=0x03 => Mbc::Mbc1 {
                rom_bank: 1,
                ram_bank: 0,
                mode: false,
                ram_enabled: false,
            },
            0x0F..=0x13 => Mbc::Mbc3 {
                rom_bank: 1,
                ram_bank: 0,
                ram_enabled: false,
            },
            0x19..=0x1E => Mbc::Mbc5 {
                rom_bank_lo: 1,
                rom_bank_hi: 0,
                ram_bank: 0,
                ram_enabled: false,
            },
            t => return Err(format!("unsupported MBC: 0x{:02X}", t)),
        };
        Ok(Cartridge {
            header,
            rom,
            sram,
            mbc,
        })
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => {
                let phys = match &self.mbc {
                    Mbc::Mbc1 { ram_bank, mode, .. } if *mode => {
                        let num_rom_banks = 2usize << (self.header.rom_size as usize);
                        let rom_mask = num_rom_banks - 1;
                        let effective = (*ram_bank as usize) << 5 & rom_mask;
                        effective * 0x4000 + addr as usize
                    }
                    _ => addr as usize,
                };
                self.rom.get(phys).copied().unwrap_or(0xFF)
            }
            0x4000..=0x7FFF => {
                let phys = match &self.mbc {
                    Mbc::None => addr as usize,
                    Mbc::Mbc1 {
                        rom_bank, ram_bank, ..
                    } => {
                        let num_rom_banks = 2usize << (self.header.rom_size as usize);
                        let rom_mask = num_rom_banks - 1;
                        let full = (*ram_bank as usize) << 5 | (*rom_bank as usize);
                        let mut effective = full & rom_mask;
                        if effective == 0 {
                            effective = 1;
                        }
                        effective * 0x4000 + (addr as usize - 0x4000)
                    }
                    Mbc::Mbc3 { rom_bank, .. } => {
                        *rom_bank as usize * 0x4000 + (addr as usize - 0x4000)
                    }
                    Mbc::Mbc5 {
                        rom_bank_lo,
                        rom_bank_hi,
                        ..
                    } => {
                        let bank = (*rom_bank_hi as usize) << 8 | (*rom_bank_lo as usize);
                        bank * 0x4000 + (addr as usize - 0x4000)
                    }
                };
                self.rom.get(phys).copied().unwrap_or(0xFF)
            }
            0xA000..=0xBFFF => {
                if self.sram.is_empty() {
                    return 0xFF;
                }
                match &self.mbc {
                    Mbc::None => 0xFF,
                    Mbc::Mbc1 {
                        ram_enabled,
                        ram_bank,
                        mode,
                        ..
                    } => {
                        if !*ram_enabled {
                            return 0xFF;
                        }
                        let bank_eff = if *mode { *ram_bank & 0x03 } else { 0 };
                        let phys = bank_eff as usize * 0x2000 + (addr as usize - 0xA000);
                        self.sram.get(phys).copied().unwrap_or(0xFF)
                    }
                    Mbc::Mbc3 {
                        ram_enabled,
                        ram_bank,
                        ..
                    } => {
                        if !*ram_enabled {
                            return 0xFF;
                        }
                        if *ram_bank >= 0x08 {
                            return 0xFF;
                        }
                        let bank_eff = *ram_bank & 0x03;
                        let phys = bank_eff as usize * 0x2000 + (addr as usize - 0xA000);
                        self.sram.get(phys).copied().unwrap_or(0xFF)
                    }
                    Mbc::Mbc5 {
                        ram_enabled,
                        ram_bank,
                        ..
                    } => {
                        if !*ram_enabled {
                            return 0xFF;
                        }
                        let bank_eff = *ram_bank & 0x0F;
                        let phys = bank_eff as usize * 0x2000 + (addr as usize - 0xA000);
                        self.sram.get(phys).copied().unwrap_or(0xFF)
                    }
                }
            }
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        if (0xA000..=0xBFFF).contains(&addr) {
            let (do_write, bank_eff) = match &self.mbc {
                Mbc::None => (false, 0u8),
                Mbc::Mbc1 {
                    ram_enabled,
                    ram_bank,
                    mode,
                    ..
                } => {
                    let eff = if *mode { *ram_bank & 0x03 } else { 0 };
                    (*ram_enabled, eff)
                }
                Mbc::Mbc3 {
                    ram_enabled,
                    ram_bank,
                    ..
                } => (*ram_enabled && *ram_bank <= 0x03, *ram_bank & 0x03),
                Mbc::Mbc5 {
                    ram_enabled,
                    ram_bank,
                    ..
                } => (*ram_enabled, *ram_bank & 0x0F),
            };
            if do_write && !self.sram.is_empty() {
                let phys = bank_eff as usize * 0x2000 + (addr as usize - 0xA000);
                if let Some(b) = self.sram.get_mut(phys) {
                    *b = val;
                }
            }
            return;
        }

        match &mut self.mbc {
            Mbc::None => {}
            Mbc::Mbc1 {
                rom_bank,
                ram_bank,
                mode,
                ram_enabled,
            } => match addr {
                0x0000..=0x1FFF => *ram_enabled = val & 0x0F == 0x0A,
                0x2000..=0x3FFF => {
                    *rom_bank = val & 0x1F;
                    if *rom_bank == 0 {
                        *rom_bank = 1;
                    }
                }
                0x4000..=0x5FFF => *ram_bank = val & 0x03,
                0x6000..=0x7FFF => *mode = val & 0x01 != 0,
                _ => {}
            },
            Mbc::Mbc3 {
                rom_bank,
                ram_bank,
                ram_enabled,
            } => match addr {
                0x0000..=0x1FFF => *ram_enabled = val & 0x0F == 0x0A,
                0x2000..=0x3FFF => {
                    *rom_bank = val & 0x7F;
                    if *rom_bank == 0 {
                        *rom_bank = 1;
                    }
                }
                0x4000..=0x5FFF => *ram_bank = val,
                _ => {}
            },
            Mbc::Mbc5 {
                rom_bank_lo,
                rom_bank_hi,
                ram_bank,
                ram_enabled,
            } => match addr {
                0x0000..=0x1FFF => *ram_enabled = val & 0x0F == 0x0A,
                0x2000..=0x2FFF => *rom_bank_lo = val,
                0x3000..=0x3FFF => *rom_bank_hi = val & 0x01,
                0x4000..=0x5FFF => *ram_bank = val & 0x0F,
                _ => {}
            },
        }
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
        let cart = Cartridge::load(rom).unwrap();
        assert_eq!(cart.header.cartridge_type, 0x01);
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

    #[test]
    fn mbc1_rom_bank_select() {
        // 128 KiB = 8 banks × 16 KiB
        let mut rom = vec![0u8; 128 * 1024];
        for (i, &b) in b"TEST".iter().enumerate() {
            rom[0x0134 + i] = b;
        }
        rom[0x0147] = 0x01; // MBC1
        rom[0x0148] = 0x02; // 128 KiB
        for bank in 0..8usize {
            rom[bank * 0x4000] = bank as u8 + 1;
        }
        let mut cart = Cartridge::load(rom).unwrap();
        for bank in 1u8..8 {
            cart.write(0x2000, bank);
            assert_eq!(cart.read(0x4000), bank + 1, "bank {bank} sentinel mismatch");
        }
    }

    #[test]
    fn mbc1_ram_bank_select() {
        let mut rom = minimal_rom(0x03); // MBC1+RAM+BATTERY
        rom[0x0149] = 0x02; // 8 KiB SRAM
        let mut cart = Cartridge::load(rom).unwrap();
        cart.write(0x0000, 0x0A); // enable SRAM
        cart.write(0xA000, 0x42);
        assert_eq!(cart.read(0xA000), 0x42);
    }

    #[test]
    fn mbc3_rom_bank_zero_becomes_one() {
        let mut rom = vec![0u8; 0x8000]; // 32 KiB
        for (i, &b) in b"TEST".iter().enumerate() {
            rom[0x0134 + i] = b;
        }
        rom[0x0147] = 0x11; // MBC3
        rom[0x4000] = 0xAB; // sentinel at bank 1
        let mut cart = Cartridge::load(rom).unwrap();
        cart.write(0x2000, 0x00); // try to select bank 0
        assert_eq!(cart.read(0x4000), 0xAB); // should read bank 1
    }

    #[test]
    fn mbc5_bank_zero_accessible() {
        let mut rom = vec![0u8; 0x8000]; // 32 KiB
        for (i, &b) in b"TEST".iter().enumerate() {
            rom[0x0134 + i] = b;
        }
        rom[0x0147] = 0x19; // MBC5
        rom[0x0000] = 0xCD; // sentinel at bank 0
        rom[0x4000] = 0xEF; // sentinel at bank 1
        let mut cart = Cartridge::load(rom).unwrap();
        cart.write(0x2000, 0x00); // select bank 0 (valid on MBC5)
        assert_eq!(cart.read(0x4000), 0xCD); // bank 0, physical 0x0000
    }

    #[test]
    fn sram_disabled_reads_ff() {
        let mut rom = minimal_rom(0x03); // MBC1+RAM
        rom[0x0149] = 0x02; // 8 KiB SRAM
        let cart = Cartridge::load(rom).unwrap();
        // SRAM not enabled — must return 0xFF
        assert_eq!(cart.read(0xA000), 0xFF);
    }

    #[test]
    fn unknown_mbc_returns_err() {
        let rom = minimal_rom(0xFF);
        let err = Cartridge::load(rom).unwrap_err();
        assert!(err.contains("unsupported MBC"), "got: {err}");
    }
}

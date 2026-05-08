use crate::cartridge::Cartridge;
use crate::ppu::Ppu;
use crate::timer::Timer;

pub struct Bus {
    cartridge: Cartridge,
    wram: [u8; 0x2000],
    hram: [u8; 0x7F],
    timer: Timer,
    pub ppu: Ppu,
    vram: [u8; 0x2000],
    oam: [u8; 0xA0],
    sb: u8,
    sc: u8,
    interrupt_flags: u8, // 0xFF0F — IF register (bits 4-0)
    ie: u8,              // 0xFFFF — IE register (bits 4-0)
    serial_buf: Vec<u8>,
}

impl Bus {
    pub fn new(cartridge: Cartridge) -> Self {
        Bus {
            cartridge,
            wram: [0; 0x2000],
            hram: [0; 0x7F],
            timer: Timer::new(),
            ppu: Ppu::new(),
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            sb: 0,
            sc: 0,
            interrupt_flags: 0,
            ie: 0,
            serial_buf: Vec::new(),
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.cartridge.read(addr),
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize],
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize],
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize],
            0xFF01 => self.sb,
            0xFF02 => self.sc,
            0xFF04..=0xFF07 => self.timer.read(addr),
            0xFF0F => self.interrupt_flags,
            0xFF40 => self.ppu.lcdc,
            0xFF41 => self.ppu.stat | 0x80,
            0xFF42 => self.ppu.scy,
            0xFF43 => self.ppu.scx,
            0xFF44 => self.ppu.ly,
            0xFF45 => self.ppu.lyc,
            0xFF46 => 0xFF,
            0xFF47 => self.ppu.bgp,
            0xFF48 => self.ppu.obp0,
            0xFF49 => self.ppu.obp1,
            0xFF4A => self.ppu.wy,
            0xFF4B => self.ppu.wx,
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.ie,
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF => {}
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize] = val,
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = val,
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize] = val,
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize] = val,
            0xFF01 => self.sb = val,
            0xFF02 => {
                if val & 0x80 != 0 {
                    self.serial_buf.push(self.sb);
                    self.sc = val & !0x80;
                    self.interrupt_flags |= 1 << 3;
                } else {
                    self.sc = val;
                }
            }
            0xFF04..=0xFF07 => self.timer.write(addr, val),
            0xFF0F => self.interrupt_flags = val & 0x1F,
            0xFF40 => self.ppu.lcdc = val,
            0xFF41 => self.ppu.stat = (self.ppu.stat & 0x07) | (val & 0xF8),
            0xFF42 => self.ppu.scy = val,
            0xFF43 => self.ppu.scx = val,
            0xFF44 => {}
            0xFF45 => self.ppu.lyc = val,
            0xFF46 => {
                let src = (val as u16) << 8;
                let mut buf = [0u8; 160];
                for i in 0..160u16 {
                    buf[i as usize] = self.read(src + i);
                }
                self.oam.copy_from_slice(&buf);
            }
            0xFF47 => self.ppu.bgp = val,
            0xFF48 => self.ppu.obp0 = val,
            0xFF49 => self.ppu.obp1 = val,
            0xFF4A => self.ppu.wy = val,
            0xFF4B => self.ppu.wx = val,
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = val,
            0xFFFF => self.ie = val & 0x1F,
            _ => {}
        }
    }

    pub fn if_reg(&self) -> u8 {
        self.interrupt_flags
    }
    pub fn ie(&self) -> u8 {
        self.ie
    }

    pub fn set_if_bit(&mut self, bit: u8) {
        self.interrupt_flags |= 1 << bit;
    }

    pub fn step_timer(&mut self, t_cycles: u32) {
        let overflows = self.timer.step(t_cycles);
        if overflows > 0 {
            self.set_if_bit(2);
        }
    }

    pub fn step_ppu(&mut self, t_cycles: u32) {
        let Bus {
            ppu,
            vram,
            oam,
            interrupt_flags,
            ..
        } = self;
        ppu.step(t_cycles, vram, oam, interrupt_flags);
    }

    pub fn take_serial_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.serial_buf)
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
    }

    #[test]
    fn rom_write_silent() {
        let mut bus = test_bus();
        let original = bus.read(0x0000);
        bus.write(0x0000, 0xAA);
        assert_eq!(bus.read(0x0000), original);
    }

    #[test]
    fn ie_register_roundtrip() {
        let mut bus = test_bus();
        bus.write(0xFFFF, 0x1F);
        assert_eq!(bus.ie(), 0x1F);
        assert_eq!(bus.read(0xFFFF), 0x1F);
    }

    #[test]
    fn if_register_roundtrip() {
        let mut bus = test_bus();
        bus.write(0xFF0F, 0x15);
        assert_eq!(bus.if_reg(), 0x15);
    }

    #[test]
    fn serial_stub_captures_byte() {
        let mut bus = test_bus();
        bus.write(0xFF01, 0x48); // SB = 'H'
        bus.write(0xFF02, 0x81); // SC = transfer start
        assert_eq!(bus.take_serial_output(), vec![0x48]);
        assert!(bus.if_reg() & 0x08 != 0); // IF bit 3 set
        assert_eq!(bus.read(0xFF02) & 0x80, 0); // SC bit 7 cleared
    }

    #[test]
    fn step_timer_sets_if_bit_2_on_overflow() {
        let mut bus = test_bus();
        bus.write(0xFF05, 0xFF); // TIMA = 0xFF (one tick away from overflow)
        bus.write(0xFF06, 0x00); // TMA = 0
        bus.write(0xFF07, 0x04); // TAC: enabled, 4096 Hz
        bus.step_timer(1024);
        assert!(bus.if_reg() & 0x04 != 0); // IF bit 2 set
    }

    #[test]
    fn test_oam_dma() {
        let mut bus = test_bus();
        for i in 0..160u16 {
            bus.write(0xC000 + i, i as u8);
        }
        bus.write(0xFF46, 0xC0);
        for i in 0..160u16 {
            assert_eq!(bus.read(0xFE00 + i), i as u8);
        }
    }
}

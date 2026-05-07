use crate::cartridge::Cartridge;
use crate::timer::Timer;

pub struct Bus {
    cartridge: Cartridge,
    wram: [u8; 0x2000],
    hram: [u8; 0x7F],
    timer: Timer,
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
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize],
            0xFF01 => self.sb,
            0xFF02 => self.sc,
            0xFF04..=0xFF07 => self.timer.read(addr),
            0xFF0F => self.interrupt_flags,
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.ie,
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF => {}
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = val,
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize] = val,
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
            0xFF46 => {} // OAM DMA stub — silently dropped until Phase 3
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
}

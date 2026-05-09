use crate::apu::Apu;
use crate::cartridge::Cartridge;
use crate::ppu::Ppu;
use crate::timer::Timer;

struct Joypad {
    pressed: u8, // 1=pressed: bit0=A bit1=B bit2=Sel bit3=Start bit4=Right bit5=Left bit6=Up bit7=Down
    select: u8,  // bits 5-4 from last write to 0xFF00; default 0x30
}

impl Joypad {
    fn new() -> Self {
        Joypad {
            pressed: 0,
            select: 0x30,
        }
    }

    fn read_output(&self) -> u8 {
        let mut result = 0x0F;
        // P15 clear (bit 5 of select) → action row selected
        if self.select & 0x20 == 0 {
            let a = self.pressed & 1;
            let b = (self.pressed >> 1) & 1;
            let sel = (self.pressed >> 2) & 1;
            let start = (self.pressed >> 3) & 1;
            let row = (start << 3) | (sel << 2) | (b << 1) | a;
            result &= (!row) & 0x0F;
        }
        // P14 clear (bit 4 of select) → direction row selected
        if self.select & 0x10 == 0 {
            let right = (self.pressed >> 4) & 1;
            let left = (self.pressed >> 5) & 1;
            let up = (self.pressed >> 6) & 1;
            let down = (self.pressed >> 7) & 1;
            let row = (down << 3) | (up << 2) | (left << 1) | right;
            result &= (!row) & 0x0F;
        }
        result
    }

    fn read_full(&self) -> u8 {
        0xC0 | (self.select & 0x30) | self.read_output()
    }
}

pub struct Bus {
    cartridge: Cartridge,
    wram: [u8; 0x2000],
    hram: [u8; 0x7F],
    timer: Timer,
    pub ppu: Ppu,
    pub apu: Apu,
    vram: [u8; 0x2000],
    oam: [u8; 0xA0],
    joypad: Joypad,
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
            apu: Apu::new(44100.0),
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            joypad: Joypad::new(),
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
            0xFF00 => self.joypad.read_full(),
            0xFF01 => self.sb,
            0xFF02 => self.sc,
            0xFF04..=0xFF07 => self.timer.read(addr),
            0xFF0F => self.interrupt_flags,
            0xFF10..=0xFF3F => self.apu.read(addr),
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
            0xA000..=0xBFFF => self.cartridge.read(addr),
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.ie,
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF => self.cartridge.write(addr, val),
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize] = val,
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = val,
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize] = val,
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize] = val,
            0xFF00 => self.joypad.select = val & 0x30,
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
            0xFF10..=0xFF3F => self.apu.write(addr, val),
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
            0xA000..=0xBFFF => self.cartridge.write(addr, val),
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

    pub fn step_apu(&mut self, t_cycles: u32, out: &mut Vec<f32>) {
        self.apu.step(t_cycles, out);
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

    pub fn set_joypad_buttons(&mut self, pressed: u8) {
        let old_output = self.joypad.read_output();
        self.joypad.pressed = pressed;
        let new_output = self.joypad.read_output();
        // Any output bit 1→0 (inactive→active) fires the joypad interrupt (IF bit 4)
        if old_output & !new_output != 0 {
            self.interrupt_flags |= 1 << 4;
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
        // select=0x30 (no row selected), no buttons pressed → 0xC0|0x30|0x0F = 0xFF
        assert_eq!(bus.read(0xFF00), 0xFF);
    }

    #[test]
    fn joypad_direction_row_read() {
        let mut bus = test_bus();
        // Select direction row: bit4 clear (P14 low), bit5 set (P15 high)
        bus.write(0xFF00, 0x20);
        bus.set_joypad_buttons(1 << 4); // Right pressed
                                        // active-low: Right=1, others 0 → row=0b0001 → !row&0x0F=0x0E
                                        // read_full = 0xC0 | 0x20 | 0x0E = 0xEE
        assert_eq!(bus.read(0xFF00), 0xEE);
    }

    #[test]
    fn joypad_action_row_read() {
        let mut bus = test_bus();
        // Select action row: bit5 clear (P15 low), bit4 set (P14 high)
        bus.write(0xFF00, 0x10);
        bus.set_joypad_buttons(1 << 0); // A pressed
                                        // active-low: A=1, others 0 → row=0b0001 → !row&0x0F=0x0E
                                        // read_full = 0xC0 | 0x10 | 0x0E = 0xDE
        assert_eq!(bus.read(0xFF00), 0xDE);
    }

    #[test]
    fn joypad_interrupt_fires_on_press() {
        let mut bus = test_bus();
        bus.write(0xFF00, 0x20); // direction row selected
        bus.set_joypad_buttons(1 << 4); // Right pressed: output 0x0F → 0x0E
        assert!(
            bus.if_reg() & 0x10 != 0,
            "joypad interrupt (IF bit 4) should fire on press"
        );
    }

    #[test]
    fn joypad_interrupt_does_not_fire_on_release() {
        let mut bus = test_bus();
        bus.write(0xFF00, 0x20);
        bus.set_joypad_buttons(1 << 4); // press
        bus.write(0xFF0F, 0x00); // clear IF
        bus.set_joypad_buttons(0); // release: output 0x0E → 0x0F (0→1, not 1→0)
        assert!(
            bus.if_reg() & 0x10 == 0,
            "joypad interrupt should not fire on release"
        );
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

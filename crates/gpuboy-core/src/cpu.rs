use crate::bus::Bus;

pub struct Cpu {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
    pub ime: bool,
    pub(crate) ime_pending: bool,
    pub(crate) halted: bool,
    pub(crate) halt_bug: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            a: 0x01,
            f: 0xB0,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
            ime: false,
            ime_pending: false,
            halted: false,
            halt_bug: false,
        }
    }

    // Flag getters (bits 7-4 of F; bits 3-0 are always 0)
    pub fn zf(&self) -> bool {
        self.f & 0x80 != 0
    }
    pub fn nf(&self) -> bool {
        self.f & 0x40 != 0
    }
    pub fn hf(&self) -> bool {
        self.f & 0x20 != 0
    }
    pub fn cf(&self) -> bool {
        self.f & 0x10 != 0
    }

    // Flag setters — bits 3-0 of F are never touched
    pub fn set_zf(&mut self, v: bool) {
        if v {
            self.f |= 0x80
        } else {
            self.f &= !0x80
        }
    }
    pub fn set_nf(&mut self, v: bool) {
        if v {
            self.f |= 0x40
        } else {
            self.f &= !0x40
        }
    }
    pub fn set_hf(&mut self, v: bool) {
        if v {
            self.f |= 0x20
        } else {
            self.f &= !0x20
        }
    }
    pub fn set_cf(&mut self, v: bool) {
        if v {
            self.f |= 0x10
        } else {
            self.f &= !0x10
        }
    }

    // Register pair getters
    pub fn af(&self) -> u16 {
        ((self.a as u16) << 8) | self.f as u16
    }
    pub fn bc(&self) -> u16 {
        ((self.b as u16) << 8) | self.c as u16
    }
    pub fn de(&self) -> u16 {
        ((self.d as u16) << 8) | self.e as u16
    }
    pub fn hl(&self) -> u16 {
        ((self.h as u16) << 8) | self.l as u16
    }

    // Register pair setters
    pub fn set_af(&mut self, v: u16) {
        self.a = (v >> 8) as u8;
        self.f = (v & 0xF0) as u8; // lower nibble always 0
    }
    pub fn set_bc(&mut self, v: u16) {
        self.b = (v >> 8) as u8;
        self.c = v as u8;
    }
    pub fn set_de(&mut self, v: u16) {
        self.d = (v >> 8) as u8;
        self.e = v as u8;
    }
    pub fn set_hl(&mut self, v: u16) {
        self.h = (v >> 8) as u8;
        self.l = v as u8;
    }

    pub fn fetch_byte(&mut self, bus: &mut Bus) -> u8 {
        let b = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        b
    }

    pub fn fetch_word(&mut self, bus: &mut Bus) -> u16 {
        let lo = self.fetch_byte(bus) as u16;
        let hi = self.fetch_byte(bus) as u16;
        (hi << 8) | lo
    }

    pub fn step(&mut self, bus: &mut Bus) -> u32 {
        // 1. HALT check
        if self.halted {
            let wakeup = (bus.if_reg() & bus.ie() & 0x1F) != 0;
            if !wakeup {
                return 4;
            }
            self.halted = false;
            // fall through to interrupt check
        }

        // 2. Interrupt dispatch
        if self.ime {
            let pending = bus.if_reg() & bus.ie() & 0x1F;
            if pending != 0 {
                let bit = pending.trailing_zeros() as u8;
                let vector = 0x0040u16 + (bit as u16) * 8;
                self.sp = self.sp.wrapping_sub(1);
                bus.write(self.sp, (self.pc >> 8) as u8);
                self.sp = self.sp.wrapping_sub(1);
                bus.write(self.sp, self.pc as u8);
                let new_if = bus.if_reg() & !(1 << bit);
                bus.write(0xFF0F, new_if);
                self.pc = vector;
                self.ime = false;
                return 20;
            }
        }

        // 3. Fetch opcode
        let opcode = if self.halt_bug {
            self.halt_bug = false;
            bus.read(self.pc) // PC does not increment
        } else {
            self.fetch_byte(bus)
        };

        // 4. Execute opcode
        let cycles = match opcode {
            0xCB => {
                let cb_op = self.fetch_byte(bus);
                self.step_cb(cb_op, bus)
            }
            _ => todo!(
                "Task 7: opcode {:#04X} at {:#06X}",
                opcode,
                self.pc.wrapping_sub(1)
            ),
        };

        // 5. EI promotion
        if self.ime_pending {
            self.ime = true;
            self.ime_pending = false;
        }

        cycles
    }

    fn step_cb(&mut self, _opcode: u8, _bus: &mut Bus) -> u32 {
        todo!("Task 8")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::Cartridge;

    fn make_cpu_bus(rom_patch: &[(u16, u8)]) -> (Cpu, Bus) {
        let mut rom = vec![0u8; 0x8000];
        rom[0x0147] = 0x00;
        for &(addr, val) in rom_patch {
            rom[addr as usize] = val;
        }
        let cart = Cartridge::load(rom).unwrap();
        (Cpu::new(), Bus::new(cart))
    }

    #[test]
    fn post_boot_state() {
        let cpu = Cpu::new();
        assert_eq!(cpu.a, 0x01);
        assert_eq!(cpu.f, 0xB0);
        assert_eq!(cpu.bc(), 0x0013);
        assert_eq!(cpu.de(), 0x00D8);
        assert_eq!(cpu.hl(), 0x014D);
        assert_eq!(cpu.sp, 0xFFFE);
        assert_eq!(cpu.pc, 0x0100);
        assert!(!cpu.ime);
    }

    #[test]
    fn flags_roundtrip() {
        let mut cpu = Cpu::new();
        cpu.f = 0x00;
        cpu.set_zf(true);
        cpu.set_nf(true);
        cpu.set_hf(true);
        cpu.set_cf(true);
        assert_eq!(cpu.f, 0xF0);
        assert!(cpu.zf() && cpu.nf() && cpu.hf() && cpu.cf());
        cpu.set_zf(false);
        cpu.set_cf(false);
        assert_eq!(cpu.f, 0x60);
        assert_eq!(cpu.f & 0x0F, 0x00); // lower nibble always 0
    }

    #[test]
    fn set_af_masks_lower_nibble() {
        let mut cpu = Cpu::new();
        cpu.set_af(0x12FF);
        assert_eq!(cpu.a, 0x12);
        assert_eq!(cpu.f, 0xF0); // lower nibble zeroed
    }

    #[test]
    fn fetch_byte_advances_pc() {
        let (mut cpu, mut bus) = make_cpu_bus(&[(0x0100, 0x42)]);
        let b = cpu.fetch_byte(&mut bus);
        assert_eq!(b, 0x42);
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn fetch_word_little_endian() {
        let (mut cpu, mut bus) = make_cpu_bus(&[(0x0100, 0x34), (0x0101, 0x12)]);
        let w = cpu.fetch_word(&mut bus);
        assert_eq!(w, 0x1234);
        assert_eq!(cpu.pc, 0x0102);
    }
}

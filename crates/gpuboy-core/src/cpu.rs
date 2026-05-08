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
            // --- 0x00–0x3F -----------------------------------------------
            0x00 => 4, // NOP

            // LD rr,nn
            0x01 => {
                let nn = self.fetch_word(bus);
                self.set_bc(nn);
                12
            }
            0x11 => {
                let nn = self.fetch_word(bus);
                self.set_de(nn);
                12
            }
            0x21 => {
                let nn = self.fetch_word(bus);
                self.set_hl(nn);
                12
            }
            0x31 => {
                self.sp = self.fetch_word(bus);
                12
            }

            // LD (BC/DE),A
            0x02 => {
                bus.write(self.bc(), self.a);
                8
            }
            0x12 => {
                bus.write(self.de(), self.a);
                8
            }

            // INC rr
            0x03 => {
                let v = self.bc().wrapping_add(1);
                self.set_bc(v);
                8
            }
            0x13 => {
                let v = self.de().wrapping_add(1);
                self.set_de(v);
                8
            }
            0x23 => {
                let v = self.hl().wrapping_add(1);
                self.set_hl(v);
                8
            }
            0x33 => {
                self.sp = self.sp.wrapping_add(1);
                8
            }

            // DEC rr
            0x0B => {
                let v = self.bc().wrapping_sub(1);
                self.set_bc(v);
                8
            }
            0x1B => {
                let v = self.de().wrapping_sub(1);
                self.set_de(v);
                8
            }
            0x2B => {
                let v = self.hl().wrapping_sub(1);
                self.set_hl(v);
                8
            }
            0x3B => {
                self.sp = self.sp.wrapping_sub(1);
                8
            }

            // INC r
            0x04 => {
                let old = self.b;
                self.b = old.wrapping_add(1);
                self.set_zf(self.b == 0);
                self.set_nf(false);
                self.set_hf((old & 0xF) == 0xF);
                4
            }
            0x0C => {
                let old = self.c;
                self.c = old.wrapping_add(1);
                self.set_zf(self.c == 0);
                self.set_nf(false);
                self.set_hf((old & 0xF) == 0xF);
                4
            }
            0x14 => {
                let old = self.d;
                self.d = old.wrapping_add(1);
                self.set_zf(self.d == 0);
                self.set_nf(false);
                self.set_hf((old & 0xF) == 0xF);
                4
            }
            0x1C => {
                let old = self.e;
                self.e = old.wrapping_add(1);
                self.set_zf(self.e == 0);
                self.set_nf(false);
                self.set_hf((old & 0xF) == 0xF);
                4
            }
            0x24 => {
                let old = self.h;
                self.h = old.wrapping_add(1);
                self.set_zf(self.h == 0);
                self.set_nf(false);
                self.set_hf((old & 0xF) == 0xF);
                4
            }
            0x2C => {
                let old = self.l;
                self.l = old.wrapping_add(1);
                self.set_zf(self.l == 0);
                self.set_nf(false);
                self.set_hf((old & 0xF) == 0xF);
                4
            }
            0x3C => {
                let old = self.a;
                self.a = old.wrapping_add(1);
                self.set_zf(self.a == 0);
                self.set_nf(false);
                self.set_hf((old & 0xF) == 0xF);
                4
            }

            // DEC r
            0x05 => {
                let old = self.b;
                self.b = old.wrapping_sub(1);
                self.set_zf(self.b == 0);
                self.set_nf(true);
                self.set_hf((old & 0xF) == 0x0);
                4
            }
            0x0D => {
                let old = self.c;
                self.c = old.wrapping_sub(1);
                self.set_zf(self.c == 0);
                self.set_nf(true);
                self.set_hf((old & 0xF) == 0x0);
                4
            }
            0x15 => {
                let old = self.d;
                self.d = old.wrapping_sub(1);
                self.set_zf(self.d == 0);
                self.set_nf(true);
                self.set_hf((old & 0xF) == 0x0);
                4
            }
            0x1D => {
                let old = self.e;
                self.e = old.wrapping_sub(1);
                self.set_zf(self.e == 0);
                self.set_nf(true);
                self.set_hf((old & 0xF) == 0x0);
                4
            }
            0x25 => {
                let old = self.h;
                self.h = old.wrapping_sub(1);
                self.set_zf(self.h == 0);
                self.set_nf(true);
                self.set_hf((old & 0xF) == 0x0);
                4
            }
            0x2D => {
                let old = self.l;
                self.l = old.wrapping_sub(1);
                self.set_zf(self.l == 0);
                self.set_nf(true);
                self.set_hf((old & 0xF) == 0x0);
                4
            }
            0x3D => {
                let old = self.a;
                self.a = old.wrapping_sub(1);
                self.set_zf(self.a == 0);
                self.set_nf(true);
                self.set_hf((old & 0xF) == 0x0);
                4
            }

            // INC/DEC (HL)
            0x34 => {
                let addr = self.hl();
                let old = bus.read(addr);
                let result = old.wrapping_add(1);
                bus.write(addr, result);
                self.set_zf(result == 0);
                self.set_nf(false);
                self.set_hf((old & 0xF) == 0xF);
                12
            }
            0x35 => {
                let addr = self.hl();
                let old = bus.read(addr);
                let result = old.wrapping_sub(1);
                bus.write(addr, result);
                self.set_zf(result == 0);
                self.set_nf(true);
                self.set_hf((old & 0xF) == 0x0);
                12
            }

            // LD r,n
            0x06 => {
                self.b = self.fetch_byte(bus);
                8
            }
            0x0E => {
                self.c = self.fetch_byte(bus);
                8
            }
            0x16 => {
                self.d = self.fetch_byte(bus);
                8
            }
            0x1E => {
                self.e = self.fetch_byte(bus);
                8
            }
            0x26 => {
                self.h = self.fetch_byte(bus);
                8
            }
            0x2E => {
                self.l = self.fetch_byte(bus);
                8
            }
            0x3E => {
                self.a = self.fetch_byte(bus);
                8
            }

            // LD (HL),n
            0x36 => {
                let n = self.fetch_byte(bus);
                bus.write(self.hl(), n);
                12
            }

            // RLCA
            0x07 => {
                let bit7 = self.a >> 7;
                self.a = (self.a << 1) | bit7;
                self.set_zf(false);
                self.set_nf(false);
                self.set_hf(false);
                self.set_cf(bit7 != 0);
                4
            }

            // RRCA
            0x0F => {
                let bit0 = self.a & 1;
                self.a = (self.a >> 1) | (bit0 << 7);
                self.set_zf(false);
                self.set_nf(false);
                self.set_hf(false);
                self.set_cf(bit0 != 0);
                4
            }

            // RLA
            0x17 => {
                let old_c = self.cf() as u8;
                let bit7 = self.a >> 7;
                self.a = (self.a << 1) | old_c;
                self.set_zf(false);
                self.set_nf(false);
                self.set_hf(false);
                self.set_cf(bit7 != 0);
                4
            }

            // RRA
            0x1F => {
                let old_c = self.cf() as u8;
                let bit0 = self.a & 1;
                self.a = (self.a >> 1) | (old_c << 7);
                self.set_zf(false);
                self.set_nf(false);
                self.set_hf(false);
                self.set_cf(bit0 != 0);
                4
            }

            // LD (a16),SP
            0x08 => {
                let addr = self.fetch_word(bus);
                bus.write(addr, self.sp as u8);
                bus.write(addr.wrapping_add(1), (self.sp >> 8) as u8);
                20
            }

            // ADD HL,rr — Z unchanged; N=0; H=carry from bit 11; C=carry from bit 15
            0x09 => {
                let hl = self.hl();
                let rr = self.bc();
                let result = hl as u32 + rr as u32;
                self.set_nf(false);
                self.set_hf((hl & 0xFFF) + (rr & 0xFFF) > 0xFFF);
                self.set_cf(result > 0xFFFF);
                self.set_hl(result as u16);
                8
            }
            0x19 => {
                let hl = self.hl();
                let rr = self.de();
                let result = hl as u32 + rr as u32;
                self.set_nf(false);
                self.set_hf((hl & 0xFFF) + (rr & 0xFFF) > 0xFFF);
                self.set_cf(result > 0xFFFF);
                self.set_hl(result as u16);
                8
            }
            0x29 => {
                let hl = self.hl();
                let rr = hl;
                let result = hl as u32 + rr as u32;
                self.set_nf(false);
                self.set_hf((hl & 0xFFF) + (rr & 0xFFF) > 0xFFF);
                self.set_cf(result > 0xFFFF);
                self.set_hl(result as u16);
                8
            }
            0x39 => {
                let hl = self.hl();
                let rr = self.sp;
                let result = hl as u32 + rr as u32;
                self.set_nf(false);
                self.set_hf((hl & 0xFFF) + (rr & 0xFFF) > 0xFFF);
                self.set_cf(result > 0xFFFF);
                self.set_hl(result as u16);
                8
            }

            // LD A,(BC/DE)
            0x0A => {
                self.a = bus.read(self.bc());
                8
            }
            0x1A => {
                self.a = bus.read(self.de());
                8
            }

            // STOP — consume mandatory padding byte, no LCD/speed-switch in Phase 2
            0x10 => {
                self.fetch_byte(bus);
                4
            }

            // JR e (unconditional)
            0x18 => {
                let e = self.fetch_byte(bus) as i8;
                self.pc = self.pc.wrapping_add(e as i16 as u16);
                12
            }

            // JR cc,e
            0x20 => {
                let e = self.fetch_byte(bus) as i8;
                if !self.zf() {
                    self.pc = self.pc.wrapping_add(e as i16 as u16);
                    12
                } else {
                    8
                }
            }
            0x28 => {
                let e = self.fetch_byte(bus) as i8;
                if self.zf() {
                    self.pc = self.pc.wrapping_add(e as i16 as u16);
                    12
                } else {
                    8
                }
            }
            0x30 => {
                let e = self.fetch_byte(bus) as i8;
                if !self.cf() {
                    self.pc = self.pc.wrapping_add(e as i16 as u16);
                    12
                } else {
                    8
                }
            }
            0x38 => {
                let e = self.fetch_byte(bus) as i8;
                if self.cf() {
                    self.pc = self.pc.wrapping_add(e as i16 as u16);
                    12
                } else {
                    8
                }
            }

            // LD (HL+),A / LD A,(HL+) / LD (HL-),A / LD A,(HL-)
            0x22 => {
                let addr = self.hl();
                bus.write(addr, self.a);
                self.set_hl(addr.wrapping_add(1));
                8
            }
            0x2A => {
                let addr = self.hl();
                self.a = bus.read(addr);
                self.set_hl(addr.wrapping_add(1));
                8
            }
            0x32 => {
                let addr = self.hl();
                bus.write(addr, self.a);
                self.set_hl(addr.wrapping_sub(1));
                8
            }
            0x3A => {
                let addr = self.hl();
                self.a = bus.read(addr);
                self.set_hl(addr.wrapping_sub(1));
                8
            }

            // DAA
            0x27 => {
                let mut a = self.a;
                let nf = self.nf();
                let hf = self.hf();
                let old_cf = self.cf();
                let mut new_cf = old_cf;
                if !nf {
                    if old_cf || a > 0x99 {
                        a = a.wrapping_add(0x60);
                        new_cf = true;
                    }
                    if hf || (a & 0x0F) > 0x09 {
                        a = a.wrapping_add(0x06);
                    }
                } else {
                    if old_cf {
                        a = a.wrapping_sub(0x60);
                    }
                    if hf {
                        a = a.wrapping_sub(0x06);
                    }
                }
                self.a = a;
                self.set_zf(a == 0);
                self.set_hf(false);
                self.set_cf(new_cf);
                4
            }

            // CPL
            0x2F => {
                self.a = !self.a;
                self.set_nf(true);
                self.set_hf(true);
                4
            }

            // SCF
            0x37 => {
                self.set_nf(false);
                self.set_hf(false);
                self.set_cf(true);
                4
            }

            // CCF
            0x3F => {
                let c = self.cf();
                self.set_nf(false);
                self.set_hf(false);
                self.set_cf(!c);
                4
            }

            // --- 0x40–0x7F: LD r,r / LD r,(HL) / LD (HL),r / HALT -------
            0x40..=0x7F => {
                if opcode == 0x76 {
                    if !self.ime && (bus.if_reg() & bus.ie() & 0x1F) != 0 {
                        self.halt_bug = true;
                    } else {
                        self.halted = true;
                    }
                    4
                } else {
                    let dst = (opcode >> 3) & 0x07;
                    let src = opcode & 0x07;
                    let val = self.reg_read(src, bus);
                    let cycles = if src == 6 || dst == 6 { 8 } else { 4 };
                    self.reg_write(dst, val, bus);
                    cycles
                }
            }

            // --- 0x80–0xBF: ALU register operations -----------------------
            // op in bits 5-3: 0=ADD 1=ADC 2=SUB 3=SBC 4=AND 5=XOR 6=OR 7=CP
            // src in bits 2-0 (same 0-7 encoding as LD r,r); 4 cycles, 8 if (HL)
            0x80..=0xBF => {
                let op = (opcode >> 3) & 0x07;
                let r = opcode & 0x07;
                let val = self.reg_read(r, bus);
                let cycles = if r == 6 { 8 } else { 4 };
                match op {
                    0 => self.alu_add(val, false),
                    1 => self.alu_add(val, true),
                    2 => self.alu_sub(val, false),
                    3 => self.alu_sub(val, true),
                    4 => self.alu_and(val),
                    5 => self.alu_xor(val),
                    6 => self.alu_or(val),
                    7 => self.alu_cp(val),
                    _ => unreachable!(),
                }
                cycles
            }

            // --- 0xC0–0xFF ------------------------------------------------

            // RET cc — 8 not-taken, 20 taken
            0xC0 => {
                if !self.zf() {
                    self.pc = self.pop_word(bus);
                    20
                } else {
                    8
                }
            }
            0xC8 => {
                if self.zf() {
                    self.pc = self.pop_word(bus);
                    20
                } else {
                    8
                }
            }
            0xD0 => {
                if !self.cf() {
                    self.pc = self.pop_word(bus);
                    20
                } else {
                    8
                }
            }
            0xD8 => {
                if self.cf() {
                    self.pc = self.pop_word(bus);
                    20
                } else {
                    8
                }
            }

            // POP rr
            0xC1 => {
                let v = self.pop_word(bus);
                self.set_bc(v);
                12
            }
            0xD1 => {
                let v = self.pop_word(bus);
                self.set_de(v);
                12
            }
            0xE1 => {
                let v = self.pop_word(bus);
                self.set_hl(v);
                12
            }
            0xF1 => {
                let v = self.pop_word(bus);
                self.set_af(v);
                12
            }

            // JP cc,nn — 12 not-taken, 16 taken
            0xC2 => {
                let nn = self.fetch_word(bus);
                if !self.zf() {
                    self.pc = nn;
                    16
                } else {
                    12
                }
            }
            0xCA => {
                let nn = self.fetch_word(bus);
                if self.zf() {
                    self.pc = nn;
                    16
                } else {
                    12
                }
            }
            0xD2 => {
                let nn = self.fetch_word(bus);
                if !self.cf() {
                    self.pc = nn;
                    16
                } else {
                    12
                }
            }
            0xDA => {
                let nn = self.fetch_word(bus);
                if self.cf() {
                    self.pc = nn;
                    16
                } else {
                    12
                }
            }

            // JP nn
            0xC3 => {
                self.pc = self.fetch_word(bus);
                16
            }

            // CALL cc,nn — 12 not-taken, 24 taken
            0xC4 => {
                let nn = self.fetch_word(bus);
                if !self.zf() {
                    self.push_word(self.pc, bus);
                    self.pc = nn;
                    24
                } else {
                    12
                }
            }
            0xCC => {
                let nn = self.fetch_word(bus);
                if self.zf() {
                    self.push_word(self.pc, bus);
                    self.pc = nn;
                    24
                } else {
                    12
                }
            }
            0xD4 => {
                let nn = self.fetch_word(bus);
                if !self.cf() {
                    self.push_word(self.pc, bus);
                    self.pc = nn;
                    24
                } else {
                    12
                }
            }
            0xDC => {
                let nn = self.fetch_word(bus);
                if self.cf() {
                    self.push_word(self.pc, bus);
                    self.pc = nn;
                    24
                } else {
                    12
                }
            }

            // CALL nn (unconditional)
            0xCD => {
                let nn = self.fetch_word(bus);
                self.push_word(self.pc, bus);
                self.pc = nn;
                24
            }

            // PUSH rr
            0xC5 => {
                let v = self.bc();
                self.push_word(v, bus);
                16
            }
            0xD5 => {
                let v = self.de();
                self.push_word(v, bus);
                16
            }
            0xE5 => {
                let v = self.hl();
                self.push_word(v, bus);
                16
            }
            0xF5 => {
                let v = self.af();
                self.push_word(v, bus);
                16
            }

            // Immediate ALU — same flag logic as 0x80–0xBF; 8 cycles
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => {
                let n = self.fetch_byte(bus);
                match (opcode >> 3) & 0x07 {
                    0 => self.alu_add(n, false),
                    1 => self.alu_add(n, true),
                    2 => self.alu_sub(n, false),
                    3 => self.alu_sub(n, true),
                    4 => self.alu_and(n),
                    5 => self.alu_xor(n),
                    6 => self.alu_or(n),
                    7 => self.alu_cp(n),
                    _ => unreachable!(),
                }
                8
            }

            // RST n — push PC, jump to vector encoded in bits 5-3
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                let vec = (opcode & 0x38) as u16;
                self.push_word(self.pc, bus);
                self.pc = vec;
                16
            }

            // RET
            0xC9 => {
                self.pc = self.pop_word(bus);
                16
            }

            // RETI — return and enable IME immediately (no delay)
            0xD9 => {
                self.pc = self.pop_word(bus);
                self.ime = true;
                16
            }

            // LDH (a8),A / LDH A,(a8)
            0xE0 => {
                let a8 = self.fetch_byte(bus);
                bus.write(0xFF00 | a8 as u16, self.a);
                12
            }
            0xF0 => {
                let a8 = self.fetch_byte(bus);
                self.a = bus.read(0xFF00 | a8 as u16);
                12
            }

            // LD (C),A / LD A,(C)
            0xE2 => {
                bus.write(0xFF00 | self.c as u16, self.a);
                8
            }
            0xF2 => {
                self.a = bus.read(0xFF00 | self.c as u16);
                8
            }

            // ADD SP,e — flags use lower byte/nibble of SP and raw e byte
            0xE8 => {
                let e = self.fetch_byte(bus);
                self.set_zf(false);
                self.set_nf(false);
                self.set_hf((self.sp & 0xF) + (e & 0xF) as u16 > 0xF);
                self.set_cf((self.sp & 0xFF) + e as u16 > 0xFF);
                self.sp = self.sp.wrapping_add(e as i8 as i16 as u16);
                16
            }

            // LD HL,SP+e — same flag logic as ADD SP,e; result to HL, SP unchanged
            0xF8 => {
                let e = self.fetch_byte(bus);
                self.set_zf(false);
                self.set_nf(false);
                self.set_hf((self.sp & 0xF) + (e & 0xF) as u16 > 0xF);
                self.set_cf((self.sp & 0xFF) + e as u16 > 0xFF);
                let result = self.sp.wrapping_add(e as i8 as i16 as u16);
                self.set_hl(result);
                12
            }

            // JP HL
            0xE9 => {
                self.pc = self.hl();
                4
            }

            // LD (a16),A / LD A,(a16)
            0xEA => {
                let addr = self.fetch_word(bus);
                bus.write(addr, self.a);
                16
            }
            0xFA => {
                let addr = self.fetch_word(bus);
                self.a = bus.read(addr);
                16
            }

            // DI / EI
            0xF3 => {
                self.ime = false;
                self.ime_pending = false;
                4
            }
            0xFB => {
                self.ime_pending = true;
                4
            }

            // LD SP,HL
            0xF9 => {
                self.sp = self.hl();
                8
            }

            // Undefined opcodes
            0xD3 | 0xDB | 0xDD | 0xE3 | 0xE4 | 0xEB | 0xEC | 0xED | 0xF4 | 0xFC | 0xFD => {
                panic!(
                    "undefined opcode {:#04X} at {:#06X}",
                    opcode,
                    self.pc.wrapping_sub(1)
                )
            }

            // --- 0xCB prefix ----------------------------------------------
            0xCB => {
                let cb_op = self.fetch_byte(bus);
                self.step_cb(cb_op, bus)
            }
        };

        // 5. EI promotion
        if self.ime_pending {
            self.ime = true;
            self.ime_pending = false;
        }

        cycles
    }

    fn step_cb(&mut self, opcode: u8, bus: &mut Bus) -> u32 {
        let r = opcode & 0x07;
        let b = (opcode >> 3) & 0x07; // bit index for BIT/RES/SET; op index for group 0
        let val = self.reg_read(r, bus);
        let is_hl = r == 6;

        match opcode >> 6 {
            // Group 0: shift / rotate operations
            0 => {
                let result = match b {
                    0 => {
                        // RLC: rotate left; old bit 7 → C and bit 0
                        let c = val >> 7;
                        self.set_cf(c != 0);
                        (val << 1) | c
                    }
                    1 => {
                        // RRC: rotate right; old bit 0 → C and bit 7
                        let c = val & 1;
                        self.set_cf(c != 0);
                        (val >> 1) | (c << 7)
                    }
                    2 => {
                        // RL: rotate left through carry
                        let old_c = self.cf() as u8;
                        self.set_cf(val >> 7 != 0);
                        (val << 1) | old_c
                    }
                    3 => {
                        // RR: rotate right through carry
                        let old_c = self.cf() as u8;
                        self.set_cf(val & 1 != 0);
                        (val >> 1) | (old_c << 7)
                    }
                    4 => {
                        // SLA: shift left arithmetic; bit 7 → C, bit 0 = 0
                        self.set_cf(val >> 7 != 0);
                        val << 1
                    }
                    5 => {
                        // SRA: shift right arithmetic; bit 0 → C, bit 7 preserved
                        self.set_cf(val & 1 != 0);
                        (val >> 1) | (val & 0x80)
                    }
                    6 => {
                        // SWAP: swap nibbles; C=0 (other flags set below)
                        self.set_cf(false);
                        val.rotate_left(4)
                    }
                    7 => {
                        // SRL: shift right logical; bit 0 → C, bit 7 = 0
                        self.set_cf(val & 1 != 0);
                        val >> 1
                    }
                    _ => unreachable!(),
                };
                self.set_zf(result == 0);
                self.set_nf(false);
                self.set_hf(false);
                self.reg_write(r, result, bus);
                if is_hl {
                    16
                } else {
                    8
                }
            }
            // Group 1: BIT — test bit; no write-back
            1 => {
                self.set_zf(val & (1 << b) == 0);
                self.set_nf(false);
                self.set_hf(true);
                if is_hl {
                    12
                } else {
                    8
                }
            }
            // Group 2: RES — clear bit; no flags
            2 => {
                self.reg_write(r, val & !(1 << b), bus);
                if is_hl {
                    16
                } else {
                    8
                }
            }
            // Group 3: SET — set bit; no flags
            3 => {
                self.reg_write(r, val | (1 << b), bus);
                if is_hl {
                    16
                } else {
                    8
                }
            }
            _ => unreachable!(),
        }
    }

    fn push_word(&mut self, val: u16, bus: &mut Bus) {
        self.sp = self.sp.wrapping_sub(1);
        bus.write(self.sp, (val >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1);
        bus.write(self.sp, val as u8);
    }

    fn pop_word(&mut self, bus: &mut Bus) -> u16 {
        let lo = bus.read(self.sp) as u16;
        self.sp = self.sp.wrapping_add(1);
        let hi = bus.read(self.sp) as u16;
        self.sp = self.sp.wrapping_add(1);
        (hi << 8) | lo
    }

    pub(crate) fn reg_read(&self, r: u8, bus: &Bus) -> u8 {
        match r {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => self.h,
            5 => self.l,
            6 => bus.read(self.hl()),
            7 => self.a,
            _ => unreachable!(),
        }
    }

    pub(crate) fn reg_write(&mut self, r: u8, val: u8, bus: &mut Bus) {
        match r {
            0 => self.b = val,
            1 => self.c = val,
            2 => self.d = val,
            3 => self.e = val,
            4 => self.h = val,
            5 => self.l = val,
            6 => bus.write(self.hl(), val),
            7 => self.a = val,
            _ => unreachable!(),
        }
    }

    // ALU helpers — shared by 0x80–0xBF register forms and 0xC6/CE/… immediate forms

    pub(crate) fn alu_add(&mut self, val: u8, with_carry: bool) {
        let c = if with_carry { self.cf() as u8 } else { 0 };
        let result = self.a as u16 + val as u16 + c as u16;
        self.set_hf((self.a & 0xF) + (val & 0xF) + c > 0xF);
        self.set_cf(result > 0xFF);
        self.a = result as u8;
        self.set_zf(self.a == 0);
        self.set_nf(false);
    }

    pub(crate) fn alu_sub(&mut self, val: u8, with_carry: bool) {
        let c = if with_carry { self.cf() as u8 } else { 0 };
        self.set_hf((self.a & 0xF) < (val & 0xF) + c);
        self.set_cf((self.a as u16) < val as u16 + c as u16);
        self.a = self.a.wrapping_sub(val).wrapping_sub(c);
        self.set_zf(self.a == 0);
        self.set_nf(true);
    }

    pub(crate) fn alu_and(&mut self, val: u8) {
        self.a &= val;
        self.set_zf(self.a == 0);
        self.set_nf(false);
        self.set_hf(true);
        self.set_cf(false);
    }

    pub(crate) fn alu_xor(&mut self, val: u8) {
        self.a ^= val;
        self.set_zf(self.a == 0);
        self.set_nf(false);
        self.set_hf(false);
        self.set_cf(false);
    }

    pub(crate) fn alu_or(&mut self, val: u8) {
        self.a |= val;
        self.set_zf(self.a == 0);
        self.set_nf(false);
        self.set_hf(false);
        self.set_cf(false);
    }

    pub(crate) fn alu_cp(&mut self, val: u8) {
        let a = self.a;
        self.set_hf((a & 0xF) < (val & 0xF));
        self.set_cf(a < val);
        self.set_zf(a == val);
        self.set_nf(true);
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

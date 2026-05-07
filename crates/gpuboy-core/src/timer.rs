pub struct Timer {
    counter: u16,
    tima: u8,
    tma: u8,
    tac: u8,
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            counter: 0,
            tima: 0,
            tma: 0,
            tac: 0,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => (self.counter >> 8) as u8,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac | 0xF8,
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF04 => self.counter = 0,
            0xFF05 => self.tima = val,
            0xFF06 => self.tma = val,
            0xFF07 => self.tac = val & 0x07,
            _ => {}
        }
    }

    // Returns the number of TIMA overflows that occurred.
    // Must loop one T-cycle at a time to correctly detect falling edges.
    pub fn step(&mut self, t_cycles: u32) -> u8 {
        let bit = self.selected_bit();
        let enabled = self.tac & 0x04 != 0;
        let mut overflows: u8 = 0;

        for _ in 0..t_cycles {
            let prev_bit = (self.counter >> bit) & 1;
            self.counter = self.counter.wrapping_add(1);
            let new_bit = (self.counter >> bit) & 1;

            if enabled && prev_bit == 1 && new_bit == 0 {
                let (new_tima, overflow) = self.tima.overflowing_add(1);
                if overflow {
                    self.tima = self.tma;
                    overflows = overflows.saturating_add(1);
                } else {
                    self.tima = new_tima;
                }
            }
        }

        overflows
    }

    fn selected_bit(&self) -> u16 {
        match self.tac & 0x03 {
            0 => 9,
            1 => 3,
            2 => 5,
            _ => 7,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn div_increments_after_256() {
        let mut t = Timer::new();
        t.step(256);
        assert_eq!(t.read(0xFF04), 1);
    }

    #[test]
    fn div_reset_on_write() {
        let mut t = Timer::new();
        t.step(512);
        t.write(0xFF04, 0x00);
        assert_eq!(t.read(0xFF04), 0);
    }

    #[test]
    fn tima_overflow_fires_interrupt() {
        let mut t = Timer::new();
        t.write(0xFF05, 0xFF); // TIMA = 0xFF (one tick away from overflow)
        t.write(0xFF06, 0x00); // TMA = 0
        t.write(0xFF07, 0x04); // TAC: enabled, 4096 Hz (bit 9, every 1024 T-cycles)
        let overflows = t.step(1024);
        assert_eq!(overflows, 1);
        assert_eq!(t.read(0xFF05), 0); // reloaded from TMA=0
    }

    #[test]
    fn timer_disabled() {
        let mut t = Timer::new();
        t.write(0xFF07, 0x00); // TAC bit 2 = 0 (disabled)
        t.step(100_000);
        assert_eq!(t.read(0xFF05), 0);
    }

    #[test]
    fn tac_upper_bits_read_as_one() {
        let t = Timer::new();
        assert_eq!(t.read(0xFF07), 0xF8);
    }
}

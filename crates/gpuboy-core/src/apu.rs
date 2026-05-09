pub struct Ch1 {
    pub sweep_period: u8,
    pub sweep_negate: bool,
    pub sweep_shift: u8,

    pub duty: u8,
    pub length_load: u8,

    pub env_initial: u8,
    pub env_add: bool,
    pub env_period: u8,

    pub freq_low: u8,
    pub freq_high: u8,
    pub length_enable: bool,

    pub enabled: bool,
    freq_timer: u16,
    duty_pos: u8,
    length_counter: u8,
    env_volume: u8,
    env_timer: u8,
    sweep_timer: u8,
    sweep_shadow: u16,
    sweep_enabled: bool,
}

pub struct Ch2 {
    pub duty: u8,
    pub length_load: u8,
    pub env_initial: u8,
    pub env_add: bool,
    pub env_period: u8,
    pub freq_low: u8,
    pub freq_high: u8,
    pub length_enable: bool,

    pub enabled: bool,
    freq_timer: u16,
    duty_pos: u8,
    length_counter: u8,
    env_volume: u8,
    env_timer: u8,
}

pub struct Ch3 {
    pub dac_on: bool,
    pub length_load: u8,
    pub output_level: u8,
    pub freq_low: u8,
    pub freq_high: u8,
    pub length_enable: bool,

    pub enabled: bool,
    freq_timer: u16,
    wave_pos: u8,
    length_counter: u16,
    pub wave_ram: [u8; 16],
}

pub struct Ch4 {
    pub length_load: u8,
    pub env_initial: u8,
    pub env_add: bool,
    pub env_period: u8,
    pub clock_shift: u8,
    pub lfsr_width: bool,
    pub divisor_code: u8,
    pub length_enable: bool,

    pub enabled: bool,
    freq_timer: u16,
    lfsr: u16,
    length_counter: u8,
    env_volume: u8,
    env_timer: u8,
}

pub struct Apu {
    pub enabled: bool,

    pub ch1: Ch1,
    pub ch2: Ch2,
    pub ch3: Ch3,
    pub ch4: Ch4,

    pub nr50: u8,
    pub nr51: u8,

    fs_cycles: u32,
    fs_step: u8,

    sample_cycles: f32,
    sample_period: f32,
}

impl Ch1 {
    fn new() -> Self {
        Ch1 {
            sweep_period: 0,
            sweep_negate: false,
            sweep_shift: 0,
            duty: 0,
            length_load: 0,
            env_initial: 0,
            env_add: false,
            env_period: 0,
            freq_low: 0,
            freq_high: 0,
            length_enable: false,
            enabled: false,
            freq_timer: 0,
            duty_pos: 0,
            length_counter: 0,
            env_volume: 0,
            env_timer: 0,
            sweep_timer: 0,
            sweep_shadow: 0,
            sweep_enabled: false,
        }
    }

    fn freq11(&self) -> u16 {
        self.freq_low as u16 | ((self.freq_high as u16 & 0x07) << 8)
    }
}

impl Ch2 {
    fn new() -> Self {
        Ch2 {
            duty: 0,
            length_load: 0,
            env_initial: 0,
            env_add: false,
            env_period: 0,
            freq_low: 0,
            freq_high: 0,
            length_enable: false,
            enabled: false,
            freq_timer: 0,
            duty_pos: 0,
            length_counter: 0,
            env_volume: 0,
            env_timer: 0,
        }
    }

    fn freq11(&self) -> u16 {
        self.freq_low as u16 | ((self.freq_high as u16 & 0x07) << 8)
    }
}

impl Ch3 {
    fn new() -> Self {
        Ch3 {
            dac_on: false,
            length_load: 0,
            output_level: 0,
            freq_low: 0,
            freq_high: 0,
            length_enable: false,
            enabled: false,
            freq_timer: 0,
            wave_pos: 0,
            length_counter: 0,
            wave_ram: [0; 16],
        }
    }

    fn freq11(&self) -> u16 {
        self.freq_low as u16 | ((self.freq_high as u16 & 0x07) << 8)
    }
}

impl Ch4 {
    fn new() -> Self {
        Ch4 {
            length_load: 0,
            env_initial: 0,
            env_add: false,
            env_period: 0,
            clock_shift: 0,
            lfsr_width: false,
            divisor_code: 0,
            length_enable: false,
            enabled: false,
            freq_timer: 0,
            lfsr: 0x7FFF,
            length_counter: 0,
            env_volume: 0,
            env_timer: 0,
        }
    }
}

fn ch4_divisor(code: u8) -> u16 {
    match code {
        0 => 8,
        1 => 16,
        2 => 32,
        3 => 48,
        4 => 64,
        5 => 80,
        6 => 96,
        _ => 112,
    }
}

impl Apu {
    pub fn new(sample_rate: f32) -> Apu {
        Apu {
            enabled: false,
            ch1: Ch1::new(),
            ch2: Ch2::new(),
            ch3: Ch3::new(),
            ch4: Ch4::new(),
            nr50: 0x77,
            nr51: 0xF3,
            fs_cycles: 0,
            fs_step: 0,
            sample_cycles: 0.0,
            sample_period: 4194304.0 / sample_rate,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF10 => {
                0x80 | (self.ch1.sweep_period << 4)
                    | ((self.ch1.sweep_negate as u8) << 3)
                    | self.ch1.sweep_shift
            }
            0xFF11 => (self.ch1.duty << 6) | 0x3F,
            0xFF12 => {
                (self.ch1.env_initial << 4) | ((self.ch1.env_add as u8) << 3) | self.ch1.env_period
            }
            0xFF13 => 0xFF,
            0xFF14 => 0xBF | ((self.ch1.length_enable as u8) << 6),
            0xFF15 => 0xFF,
            0xFF16 => (self.ch2.duty << 6) | 0x3F,
            0xFF17 => {
                (self.ch2.env_initial << 4) | ((self.ch2.env_add as u8) << 3) | self.ch2.env_period
            }
            0xFF18 => 0xFF,
            0xFF19 => 0xBF | ((self.ch2.length_enable as u8) << 6),
            0xFF1A => 0x7F | ((self.ch3.dac_on as u8) << 7),
            0xFF1B => 0xFF,
            0xFF1C => 0x9F | (self.ch3.output_level << 5),
            0xFF1D => 0xFF,
            0xFF1E => 0xBF | ((self.ch3.length_enable as u8) << 6),
            0xFF1F => 0xFF,
            0xFF20 => 0xFF,
            0xFF21 => {
                (self.ch4.env_initial << 4) | ((self.ch4.env_add as u8) << 3) | self.ch4.env_period
            }
            0xFF22 => {
                (self.ch4.clock_shift << 4)
                    | ((self.ch4.lfsr_width as u8) << 3)
                    | self.ch4.divisor_code
            }
            0xFF23 => 0xBF | ((self.ch4.length_enable as u8) << 6),
            0xFF24 => self.nr50,
            0xFF25 => self.nr51,
            0xFF26 => {
                0x70 | ((self.enabled as u8) << 7)
                    | ((self.ch4.enabled as u8) << 3)
                    | ((self.ch3.enabled as u8) << 2)
                    | ((self.ch2.enabled as u8) << 1)
                    | (self.ch1.enabled as u8)
            }
            0xFF27..=0xFF2F => 0xFF,
            0xFF30..=0xFF3F => self.ch3.wave_ram[(addr - 0xFF30) as usize],
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        if !self.enabled && addr != 0xFF26 {
            return;
        }
        match addr {
            0xFF10 => {
                self.ch1.sweep_period = (val >> 4) & 0x07;
                self.ch1.sweep_negate = val & 0x08 != 0;
                self.ch1.sweep_shift = val & 0x07;
            }
            0xFF11 => {
                self.ch1.duty = (val >> 6) & 0x03;
                self.ch1.length_load = val & 0x3F;
                self.ch1.length_counter = 64 - self.ch1.length_load;
            }
            0xFF12 => {
                self.ch1.env_initial = (val >> 4) & 0x0F;
                self.ch1.env_add = val & 0x08 != 0;
                self.ch1.env_period = val & 0x07;
                if val & 0xF8 == 0 {
                    self.ch1.enabled = false;
                }
            }
            0xFF13 => self.ch1.freq_low = val,
            0xFF14 => {
                self.ch1.freq_high = val & 0x07;
                self.ch1.length_enable = val & 0x40 != 0;
                if val & 0x80 != 0 {
                    self.trigger_ch1();
                }
            }
            0xFF15 => {}
            0xFF16 => {
                self.ch2.duty = (val >> 6) & 0x03;
                self.ch2.length_load = val & 0x3F;
                self.ch2.length_counter = 64 - self.ch2.length_load;
            }
            0xFF17 => {
                self.ch2.env_initial = (val >> 4) & 0x0F;
                self.ch2.env_add = val & 0x08 != 0;
                self.ch2.env_period = val & 0x07;
                if val & 0xF8 == 0 {
                    self.ch2.enabled = false;
                }
            }
            0xFF18 => self.ch2.freq_low = val,
            0xFF19 => {
                self.ch2.freq_high = val & 0x07;
                self.ch2.length_enable = val & 0x40 != 0;
                if val & 0x80 != 0 {
                    self.trigger_ch2();
                }
            }
            0xFF1A => {
                self.ch3.dac_on = val & 0x80 != 0;
                if !self.ch3.dac_on {
                    self.ch3.enabled = false;
                }
            }
            0xFF1B => {
                self.ch3.length_load = val;
                self.ch3.length_counter = 256 - val as u16;
            }
            0xFF1C => self.ch3.output_level = (val >> 5) & 0x03,
            0xFF1D => self.ch3.freq_low = val,
            0xFF1E => {
                self.ch3.freq_high = val & 0x07;
                self.ch3.length_enable = val & 0x40 != 0;
                if val & 0x80 != 0 {
                    self.trigger_ch3();
                }
            }
            0xFF1F => {}
            0xFF20 => {
                self.ch4.length_load = val & 0x3F;
                self.ch4.length_counter = 64 - self.ch4.length_load;
            }
            0xFF21 => {
                self.ch4.env_initial = (val >> 4) & 0x0F;
                self.ch4.env_add = val & 0x08 != 0;
                self.ch4.env_period = val & 0x07;
                if val & 0xF8 == 0 {
                    self.ch4.enabled = false;
                }
            }
            0xFF22 => {
                self.ch4.clock_shift = (val >> 4) & 0x0F;
                self.ch4.lfsr_width = val & 0x08 != 0;
                self.ch4.divisor_code = val & 0x07;
            }
            0xFF23 => {
                self.ch4.length_enable = val & 0x40 != 0;
                if val & 0x80 != 0 {
                    self.trigger_ch4();
                }
            }
            0xFF24 => self.nr50 = val,
            0xFF25 => self.nr51 = val,
            0xFF26 => {
                let new_enabled = val & 0x80 != 0;
                if self.enabled && !new_enabled {
                    self.ch1.enabled = false;
                    self.ch2.enabled = false;
                    self.ch3.enabled = false;
                    self.ch4.enabled = false;
                    self.ch1.freq_timer = 0;
                    self.ch2.freq_timer = 0;
                    self.ch3.freq_timer = 0;
                    self.ch4.freq_timer = 0;
                    self.ch1.env_volume = 0;
                    self.ch2.env_volume = 0;
                    self.ch4.env_volume = 0;
                    self.fs_cycles = 0;
                    self.fs_step = 0;
                }
                self.enabled = new_enabled;
            }
            0xFF27..=0xFF2F => {}
            0xFF30..=0xFF3F => self.ch3.wave_ram[(addr - 0xFF30) as usize] = val,
            _ => {}
        }
    }

    fn trigger_ch1(&mut self) {
        self.ch1.enabled = true;
        if self.ch1.length_counter == 0 {
            self.ch1.length_counter = 64;
        }
        let freq = self.ch1.freq11();
        self.ch1.freq_timer = (2048 - freq) * 4;
        self.ch1.env_volume = self.ch1.env_initial;
        self.ch1.env_timer = if self.ch1.env_period == 0 {
            8
        } else {
            self.ch1.env_period
        };
        self.ch1.sweep_shadow = freq;
        self.ch1.sweep_timer = if self.ch1.sweep_period == 0 {
            8
        } else {
            self.ch1.sweep_period
        };
        self.ch1.sweep_enabled = self.ch1.sweep_period > 0 || self.ch1.sweep_shift > 0;
        if self.ch1.sweep_enabled {
            self.sweep_calc_overflow_check();
        }
        if self.ch1.env_initial == 0 && !self.ch1.env_add {
            self.ch1.enabled = false;
        }
    }

    fn sweep_calc_overflow_check(&mut self) {
        let delta = self.ch1.sweep_shadow >> self.ch1.sweep_shift;
        let new_freq = if self.ch1.sweep_negate {
            self.ch1.sweep_shadow.wrapping_sub(delta)
        } else {
            self.ch1.sweep_shadow + delta
        };
        if new_freq > 0x7FF {
            self.ch1.enabled = false;
        }
    }

    fn trigger_ch2(&mut self) {
        self.ch2.enabled = true;
        if self.ch2.length_counter == 0 {
            self.ch2.length_counter = 64;
        }
        let freq = self.ch2.freq11();
        self.ch2.freq_timer = (2048 - freq) * 4;
        self.ch2.env_volume = self.ch2.env_initial;
        self.ch2.env_timer = if self.ch2.env_period == 0 {
            8
        } else {
            self.ch2.env_period
        };
        if self.ch2.env_initial == 0 && !self.ch2.env_add {
            self.ch2.enabled = false;
        }
    }

    fn trigger_ch3(&mut self) {
        if !self.ch3.dac_on {
            self.ch3.enabled = false;
            return;
        }
        self.ch3.enabled = true;
        if self.ch3.length_counter == 0 {
            self.ch3.length_counter = 256;
        }
        let freq = self.ch3.freq11();
        self.ch3.freq_timer = (2048 - freq) * 2;
        self.ch3.wave_pos = 0;
    }

    fn trigger_ch4(&mut self) {
        self.ch4.enabled = true;
        if self.ch4.length_counter == 0 {
            self.ch4.length_counter = 64;
        }
        let div = ch4_divisor(self.ch4.divisor_code);
        self.ch4.freq_timer = ((div as u32) << self.ch4.clock_shift) as u16;
        self.ch4.env_volume = self.ch4.env_initial;
        self.ch4.env_timer = if self.ch4.env_period == 0 {
            8
        } else {
            self.ch4.env_period
        };
        self.ch4.lfsr = 0x7FFF;
        if self.ch4.env_initial == 0 && !self.ch4.env_add {
            self.ch4.enabled = false;
        }
    }

    pub fn step(&mut self, t_cycles: u32, out: &mut Vec<f32>) {
        for _ in 0..t_cycles {
            if self.enabled {
                // Clock CH1 frequency timer
                if self.ch1.freq_timer > 0 {
                    self.ch1.freq_timer -= 1;
                    if self.ch1.freq_timer == 0 {
                        let freq = self.ch1.freq11();
                        self.ch1.freq_timer = (2048 - freq) * 4;
                        self.ch1.duty_pos = (self.ch1.duty_pos + 1) % 8;
                    }
                }

                // Clock CH2 frequency timer
                if self.ch2.freq_timer > 0 {
                    self.ch2.freq_timer -= 1;
                    if self.ch2.freq_timer == 0 {
                        let freq = self.ch2.freq11();
                        self.ch2.freq_timer = (2048 - freq) * 4;
                        self.ch2.duty_pos = (self.ch2.duty_pos + 1) % 8;
                    }
                }

                // Clock CH3 frequency timer
                if self.ch3.freq_timer > 0 {
                    self.ch3.freq_timer -= 1;
                    if self.ch3.freq_timer == 0 {
                        let freq = self.ch3.freq11();
                        self.ch3.freq_timer = (2048 - freq) * 2;
                        self.ch3.wave_pos = (self.ch3.wave_pos + 1) % 32;
                    }
                }

                // Clock CH4 frequency timer
                if self.ch4.freq_timer > 0 {
                    self.ch4.freq_timer -= 1;
                    if self.ch4.freq_timer == 0 {
                        let div = ch4_divisor(self.ch4.divisor_code);
                        self.ch4.freq_timer = ((div as u32) << self.ch4.clock_shift) as u16;
                        let xor = (self.ch4.lfsr & 1) ^ ((self.ch4.lfsr >> 1) & 1);
                        self.ch4.lfsr = (self.ch4.lfsr >> 1) | (xor << 14);
                        if self.ch4.lfsr_width {
                            self.ch4.lfsr = (self.ch4.lfsr & !(1 << 6)) | (xor << 6);
                        }
                    }
                }

                // Frame sequencer
                self.fs_cycles += 1;
                if self.fs_cycles >= 8192 {
                    self.fs_cycles -= 8192;
                    self.tick_frame_sequencer();
                }
            }

            // Sample generation
            self.sample_cycles += 1.0;
            if self.sample_cycles >= self.sample_period {
                self.sample_cycles -= self.sample_period;
                if self.enabled {
                    self.mix_sample(out);
                } else {
                    out.push(0.0);
                    out.push(0.0);
                }
            }
        }
    }

    fn tick_frame_sequencer(&mut self) {
        let step = self.fs_step;
        self.fs_step = (self.fs_step + 1) % 8;

        if step == 0 || step == 2 || step == 4 || step == 6 {
            self.tick_length();
        }
        if step == 2 || step == 6 {
            self.tick_sweep();
        }
        if step == 7 {
            self.tick_envelope();
        }
    }

    fn tick_length(&mut self) {
        if self.ch1.length_enable && self.ch1.length_counter > 0 {
            self.ch1.length_counter -= 1;
            if self.ch1.length_counter == 0 {
                self.ch1.enabled = false;
            }
        }
        if self.ch2.length_enable && self.ch2.length_counter > 0 {
            self.ch2.length_counter -= 1;
            if self.ch2.length_counter == 0 {
                self.ch2.enabled = false;
            }
        }
        if self.ch3.length_enable && self.ch3.length_counter > 0 {
            self.ch3.length_counter -= 1;
            if self.ch3.length_counter == 0 {
                self.ch3.enabled = false;
            }
        }
        if self.ch4.length_enable && self.ch4.length_counter > 0 {
            self.ch4.length_counter -= 1;
            if self.ch4.length_counter == 0 {
                self.ch4.enabled = false;
            }
        }
    }

    fn tick_envelope(&mut self) {
        tick_env_volume(
            self.ch1.env_period,
            &mut self.ch1.env_timer,
            self.ch1.env_add,
            &mut self.ch1.env_volume,
        );
        tick_env_volume(
            self.ch2.env_period,
            &mut self.ch2.env_timer,
            self.ch2.env_add,
            &mut self.ch2.env_volume,
        );
        tick_env_volume(
            self.ch4.env_period,
            &mut self.ch4.env_timer,
            self.ch4.env_add,
            &mut self.ch4.env_volume,
        );
    }

    fn tick_sweep(&mut self) {
        if self.ch1.sweep_timer > 0 {
            self.ch1.sweep_timer -= 1;
        }
        if self.ch1.sweep_timer == 0 {
            self.ch1.sweep_timer = if self.ch1.sweep_period == 0 {
                8
            } else {
                self.ch1.sweep_period
            };
            if self.ch1.sweep_enabled && self.ch1.sweep_period > 0 {
                let new_freq = self.sweep_calc();
                if new_freq <= 0x7FF && self.ch1.sweep_shift > 0 {
                    self.ch1.sweep_shadow = new_freq;
                    self.ch1.freq_low = (new_freq & 0xFF) as u8;
                    self.ch1.freq_high =
                        (self.ch1.freq_high & !0x07) | ((new_freq >> 8) as u8 & 0x07);
                    self.sweep_calc();
                }
            }
        }
    }

    fn sweep_calc(&mut self) -> u16 {
        let delta = self.ch1.sweep_shadow >> self.ch1.sweep_shift;
        let new_freq = if self.ch1.sweep_negate {
            self.ch1.sweep_shadow.wrapping_sub(delta)
        } else {
            self.ch1.sweep_shadow + delta
        };
        if new_freq > 0x7FF {
            self.ch1.enabled = false;
        }
        new_freq
    }

    fn mix_sample(&self, out: &mut Vec<f32>) {
        let ch1_out = if self.ch1.enabled {
            duty_sample(self.ch1.duty, self.ch1.duty_pos) * (self.ch1.env_volume as f32 / 15.0)
        } else {
            0.0
        };
        let ch2_out = if self.ch2.enabled {
            duty_sample(self.ch2.duty, self.ch2.duty_pos) * (self.ch2.env_volume as f32 / 15.0)
        } else {
            0.0
        };
        let ch3_out = if self.ch3.enabled && self.ch3.dac_on {
            wave_sample(&self.ch3.wave_ram, self.ch3.wave_pos, self.ch3.output_level)
        } else {
            0.0
        };
        let ch4_out = if self.ch4.enabled {
            noise_sample(self.ch4.lfsr) * (self.ch4.env_volume as f32 / 15.0)
        } else {
            0.0
        };

        let left_vol_scale = (((self.nr50 >> 4) & 0x07) as f32 + 1.0) / 8.0;
        let right_vol_scale = ((self.nr50 & 0x07) as f32 + 1.0) / 8.0;

        let left = (ch1_out * pan(self.nr51, 4)
            + ch2_out * pan(self.nr51, 5)
            + ch3_out * pan(self.nr51, 6)
            + ch4_out * pan(self.nr51, 7))
            / 4.0
            * left_vol_scale;
        let right = (ch1_out * pan(self.nr51, 0)
            + ch2_out * pan(self.nr51, 1)
            + ch3_out * pan(self.nr51, 2)
            + ch4_out * pan(self.nr51, 3))
            / 4.0
            * right_vol_scale;

        out.push(left);
        out.push(right);
    }
}

fn tick_env_volume(period: u8, timer: &mut u8, add: bool, volume: &mut u8) {
    if period > 0 {
        *timer -= 1;
        if *timer == 0 {
            *timer = period;
            if add && *volume < 15 {
                *volume += 1;
            } else if !add && *volume > 0 {
                *volume -= 1;
            }
        }
    }
}

fn pan(nr51: u8, bit: u8) -> f32 {
    if (nr51 >> bit) & 1 == 1 {
        1.0
    } else {
        0.0
    }
}

const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 1, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0],
];

fn duty_sample(duty: u8, pos: u8) -> f32 {
    DUTY_TABLE[(duty & 0x03) as usize][(pos & 0x07) as usize] as f32
}

fn wave_sample(wave_ram: &[u8; 16], pos: u8, output_level: u8) -> f32 {
    let pos = pos as usize;
    let raw = if pos.is_multiple_of(2) {
        wave_ram[pos / 2] >> 4
    } else {
        wave_ram[pos / 2] & 0x0F
    };
    let shifted = match output_level {
        0 => 0,
        1 => raw,
        2 => raw >> 1,
        3 => raw >> 2,
        _ => 0,
    };
    shifted as f32 / 15.0
}

fn noise_sample(lfsr: u16) -> f32 {
    if lfsr & 1 == 0 {
        1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apu_generates_samples() {
        let mut apu = Apu::new(44100.0);
        apu.enabled = true;
        apu.nr50 = 0x77;
        apu.nr51 = 0xFF;
        apu.write(0xFF11, 0x80); // duty=2, length=0
        apu.write(0xFF12, 0xF0); // env initial=15, add=false, period=0
        apu.write(0xFF13, 0x00);
        apu.write(0xFF14, 0x87); // trigger + freq high = 7
        let mut out = Vec::new();
        apu.step(10000, &mut out);
        assert!(out.len() >= 200, "expected at least 100 stereo pairs");
        assert!(
            out.iter().any(|&s| s > 0.0),
            "expected non-zero samples from CH1"
        );
    }
}

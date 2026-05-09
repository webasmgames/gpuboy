pub mod apu;
pub mod bus;
pub mod cartridge;
pub mod cpu;
pub mod ppu;
pub mod timer;

use bus::Bus;
use cartridge::Cartridge;
use cpu::Cpu;

pub struct Emulator {
    pub cpu: Cpu,
    pub bus: Bus,
}

impl Emulator {
    pub fn new(rom: Vec<u8>) -> Result<Self, String> {
        Ok(Emulator {
            cpu: Cpu::new(),
            bus: Bus::new(Cartridge::load(rom)?),
        })
    }

    pub fn step(&mut self) -> u32 {
        let t_cycles = self.cpu.step(&mut self.bus);
        self.bus.step_timer(t_cycles);
        self.bus.step_ppu(t_cycles);
        t_cycles
    }

    pub fn get_framebuffer(&self) -> &[u8] {
        self.bus.ppu.get_framebuffer()
    }

    pub fn step_frame(&mut self) {
        let mut total: u32 = 0;
        while total < 70224 {
            total += self.step();
        }
    }

    pub fn step_samples(&mut self, n: usize) -> Vec<f32> {
        let mut out = Vec::with_capacity(n * 2);
        while out.len() < n * 2 {
            let t = self.cpu.step(&mut self.bus);
            self.bus.step_timer(t);
            self.bus.step_ppu(t);
            self.bus.step_apu(t, &mut out);
        }
        out.truncate(n * 2);
        out
    }

    pub fn take_serial_output(&mut self) -> Vec<u8> {
        self.bus.take_serial_output()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_integration() {
        let mut rom = vec![0u8; 0x8000];
        rom[0x0147] = 0x00;
        let opcodes: &[u8] = &[
            0x3E, 0x48, // LD A,'H'
            0xE0, 0x01, // LDH (0xFF01),A  — SB = 'H'
            0x3E, 0x81, // LD A,0x81
            0xE0, 0x02, // LDH (0xFF02),A  — trigger transfer, captures 'H'
            0x3E, 0x69, // LD A,'i'
            0xE0, 0x01, // LDH (0xFF01),A  — SB = 'i'
            0x3E, 0x81, // LD A,0x81
            0xE0, 0x02, // LDH (0xFF02),A  — trigger transfer, captures 'i'
            0x18, 0xFE, // JR -2 (infinite loop)
        ];
        rom[0x0100..0x0100 + opcodes.len()].copy_from_slice(opcodes);
        let mut emu = Emulator::new(rom).unwrap();
        emu.step_frame();
        assert_eq!(emu.take_serial_output(), vec![0x48, 0x69]);
    }
}

fn rom(rel: &str) -> Vec<u8> {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::read(base.join("../../tests/roms").join(rel))
        .unwrap_or_else(|_| panic!("ROM not found: {rel}"))
}

fn run_mooneye(bytes: &[u8], timeout_frames: u32) -> Vec<u8> {
    let mut emu = gpuboy_core::Emulator::new(bytes.to_vec()).expect("load");
    let mut serial = Vec::new();
    for _ in 0..timeout_frames {
        emu.step_frame();
        serial.extend(emu.take_serial_output());
        if serial.len() >= 6 {
            break;
        }
    }
    serial
}

const PASS: [u8; 6] = [3, 5, 8, 13, 21, 34];

// ── acceptance (top-level) ────────────────────────────────────────────────────

#[test]
#[ignore] // cycle-precise ADD SP,e timing
fn add_sp_e_timing() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/add_sp_e_timing.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // boot ROM not implemented
fn boot_div2_s() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/boot_div2-S.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // boot ROM not implemented
fn boot_div_dmg0() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/boot_div-dmg0.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // boot ROM not implemented
fn boot_div_dmgabcmgb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/boot_div-dmgABCmgb.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // boot ROM not implemented
fn boot_div_s() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/boot_div-S.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // boot ROM not implemented
fn boot_hwio_dmg0() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/boot_hwio-dmg0.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // boot ROM not implemented
fn boot_hwio_dmgabcmgb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/boot_hwio-dmgABCmgb.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // boot ROM not implemented
fn boot_hwio_s() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/boot_hwio-S.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // boot ROM not implemented
fn boot_regs_dmg0() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/boot_regs-dmg0.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // boot ROM not implemented
fn boot_regs_dmgabc() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/boot_regs-dmgABC.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // boot ROM not implemented
fn boot_regs_mgb() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/boot_regs-mgb.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // boot ROM not implemented
fn boot_regs_sgb2() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/boot_regs-sgb2.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // boot ROM not implemented
fn boot_regs_sgb() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/boot_regs-sgb.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // cycle-precise CALL timing
fn call_cc_timing2() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/call_cc_timing2.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // cycle-precise CALL timing
fn call_cc_timing() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/call_cc_timing.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // cycle-precise CALL timing
fn call_timing2() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/call_timing2.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // cycle-precise CALL timing
fn call_timing() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/call_timing.gb"), 120)[..6],
        PASS
    );
}

#[test]
fn di_timing_gs() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/di_timing-GS.gb"), 120)[..6],
        PASS
    );
}

#[test]
fn div_timing() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/div_timing.gb"), 120)[..6],
        PASS
    );
}

#[test]
fn ei_sequence() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/ei_sequence.gb"), 120)[..6],
        PASS
    );
}

#[test]
fn ei_timing() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/ei_timing.gb"), 120)[..6],
        PASS
    );
}

#[test]
fn halt_ime0_ei() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/halt_ime0_ei.gb"), 120)[..6],
        PASS
    );
}

#[test]
fn halt_ime0_nointr_timing() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/halt_ime0_nointr_timing.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
fn halt_ime1_timing2_gs() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/halt_ime1_timing2-GS.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
fn halt_ime1_timing() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/halt_ime1_timing.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // IF/IE register edge cases
fn if_ie_registers() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/if_ie_registers.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
fn intr_timing() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/intr_timing.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // cycle-precise JP timing
fn jp_cc_timing() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/jp_cc_timing.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // cycle-precise JP timing
fn jp_timing() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/jp_timing.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // cycle-precise LD HL,SP+e timing
fn ld_hl_sp_e_timing() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/ld_hl_sp_e_timing.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // OAM DMA restart timing
fn oam_dma_restart() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/oam_dma_restart.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // OAM DMA start timing
fn oam_dma_start() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/oam_dma_start.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // OAM DMA timing
fn oam_dma_timing() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/oam_dma_timing.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // cycle-precise stack timing
fn pop_timing() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/pop_timing.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // cycle-precise stack timing
fn push_timing() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/push_timing.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // rapid DI/EI interrupt masking
fn rapid_di_ei() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/rapid_di_ei.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // cycle-precise RET cc timing
fn ret_cc_timing() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/ret_cc_timing.gb"), 120)[..6],
        PASS
    );
}

#[test]
fn reti_intr_timing() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/reti_intr_timing.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // cycle-precise RETI timing
fn reti_timing() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/reti_timing.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // cycle-precise RET timing
fn ret_timing() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/ret_timing.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // cycle-precise RST timing
fn rst_timing() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/rst_timing.gb"), 120)[..6],
        PASS
    );
}

// ── acceptance/bits ───────────────────────────────────────────────────────────

#[test]
fn bits_mem_oam() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/bits/mem_oam.gb"), 120)[..6],
        PASS
    );
}

#[test]
fn bits_reg_f() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/bits/reg_f.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // unused I/O register bit behavior
fn bits_unused_hwio_gs() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/bits/unused_hwio-GS.gb"),
            120
        )[..6],
        PASS
    );
}

// ── acceptance/instr ──────────────────────────────────────────────────────────

#[test]
fn instr_daa() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/instr/daa.gb"), 120)[..6],
        PASS
    );
}

// ── acceptance/interrupts ─────────────────────────────────────────────────────

#[test]
#[ignore] // interrupt IE during dispatch
fn interrupts_ie_push() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/interrupts/ie_push.gb"),
            120
        )[..6],
        PASS
    );
}

// ── acceptance/oam_dma ────────────────────────────────────────────────────────

#[test]
fn oam_dma_basic() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/oam_dma/basic.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // OAM DMA register read
fn oam_dma_reg_read() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/oam_dma/reg_read.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // OAM DMA source restrictions
fn oam_dma_sources_gs() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/oam_dma/sources-GS.gb"),
            120
        )[..6],
        PASS
    );
}

// ── acceptance/ppu ────────────────────────────────────────────────────────────

#[test]
#[ignore] // HBlank LY/SCX timing
fn ppu_hblank_ly_scx_timing_gs() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/ppu/hblank_ly_scx_timing-GS.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
fn ppu_intr_1_2_timing_gs() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/ppu/intr_1_2_timing-GS.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
fn ppu_intr_2_0_timing() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/ppu/intr_2_0_timing.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // STAT mode 0 interrupt timing
fn ppu_intr_2_mode0_timing() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/ppu/intr_2_mode0_timing.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // STAT mode 0 timing with sprites
fn ppu_intr_2_mode0_timing_sprites() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/ppu/intr_2_mode0_timing_sprites.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // STAT mode 3 interrupt timing
fn ppu_intr_2_mode3_timing() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/ppu/intr_2_mode3_timing.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // OAM access OK timing
fn ppu_intr_2_oam_ok_timing() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/ppu/intr_2_oam_ok_timing.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // LCD enable timing
fn ppu_lcdon_timing_gs() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/ppu/lcdon_timing-GS.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // LCD enable write timing
fn ppu_lcdon_write_timing_gs() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/ppu/lcdon_write_timing-GS.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // STAT IRQ blocking
fn ppu_stat_irq_blocking() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/ppu/stat_irq_blocking.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // LYC compare on/off behavior
fn ppu_stat_lyc_onoff() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/ppu/stat_lyc_onoff.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // VBlank STAT interrupt timing
fn ppu_vblank_stat_intr_gs() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/ppu/vblank_stat_intr-GS.gb"),
            120
        )[..6],
        PASS
    );
}

// ── acceptance/serial ─────────────────────────────────────────────────────────

#[test]
#[ignore] // serial clock alignment (boot ROM dependent)
fn serial_boot_sclk_align_dmgabcmgb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/serial/boot_sclk_align-dmgABCmgb.gb"),
            120
        )[..6],
        PASS
    );
}

// ── acceptance/timer ──────────────────────────────────────────────────────────

#[test]
fn timer_div_write() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/timer/div_write.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // timer rapid toggle edge case
fn timer_rapid_toggle() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/timer/rapid_toggle.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // timer/DIV reset interaction
fn timer_tim00_div_trigger() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/timer/tim00_div_trigger.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
fn timer_tim00() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/timer/tim00.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // timer/DIV reset interaction
fn timer_tim01_div_trigger() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/timer/tim01_div_trigger.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
fn timer_tim01() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/timer/tim01.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // timer/DIV reset interaction
fn timer_tim10_div_trigger() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/timer/tim10_div_trigger.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
fn timer_tim10() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/timer/tim10.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // timer/DIV reset interaction
fn timer_tim11_div_trigger() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/timer/tim11_div_trigger.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
fn timer_tim11() {
    assert_eq!(
        run_mooneye(&rom("mooneye-test-suite/acceptance/timer/tim11.gb"), 120)[..6],
        PASS
    );
}

#[test]
#[ignore] // TIMA reload timing
fn timer_tima_reload() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/timer/tima_reload.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // TIMA write during reload
fn timer_tima_write_reloading() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/timer/tima_write_reloading.gb"),
            120
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // TMA write during reload
fn timer_tma_write_reloading() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/acceptance/timer/tma_write_reloading.gb"),
            120
        )[..6],
        PASS
    );
}

// ── emulator-only/mbc1 ────────────────────────────────────────────────────────

#[test]
fn mbc1_bits_bank1() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc1/bits_bank1.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
fn mbc1_bits_bank2() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc1/bits_bank2.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
fn mbc1_bits_mode() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc1/bits_mode.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // MBC1 RAM gate control
fn mbc1_bits_ramg() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc1/bits_ramg.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // MBC1 multicart detection
fn mbc1_multicart_rom_8mb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc1/multicart_rom_8Mb.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
fn mbc1_ram_256kb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc1/ram_256kb.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // MBC1 RAM banking
fn mbc1_ram_64kb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc1/ram_64kb.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
fn mbc1_rom_16mb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc1/rom_16Mb.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // MBC1 ROM banking
fn mbc1_rom_1mb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc1/rom_1Mb.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // MBC1 ROM banking
fn mbc1_rom_2mb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc1/rom_2Mb.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
fn mbc1_rom_4mb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc1/rom_4Mb.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // MBC1 ROM banking
fn mbc1_rom_512kb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc1/rom_512kb.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
fn mbc1_rom_8mb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc1/rom_8Mb.gb"),
            300
        )[..6],
        PASS
    );
}

// ── emulator-only/mbc5 ────────────────────────────────────────────────────────

#[test]
#[ignore] // MBC5 ROM banking
fn mbc5_rom_16mb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc5/rom_16Mb.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // MBC5 ROM banking
fn mbc5_rom_1mb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc5/rom_1Mb.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // MBC5 ROM banking
fn mbc5_rom_2mb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc5/rom_2Mb.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
fn mbc5_rom_32mb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc5/rom_32Mb.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // MBC5 ROM banking
fn mbc5_rom_4mb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc5/rom_4Mb.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // MBC5 ROM banking
fn mbc5_rom_512kb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc5/rom_512kb.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
fn mbc5_rom_64mb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc5/rom_64Mb.gb"),
            300
        )[..6],
        PASS
    );
}

#[test]
#[ignore] // MBC5 ROM banking
fn mbc5_rom_8mb() {
    assert_eq!(
        run_mooneye(
            &rom("mooneye-test-suite/emulator-only/mbc5/rom_8Mb.gb"),
            300
        )[..6],
        PASS
    );
}

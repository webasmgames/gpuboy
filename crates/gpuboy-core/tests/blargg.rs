fn rom(rel: &str) -> Vec<u8> {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::read(base.join("../../tests/roms").join(rel))
        .unwrap_or_else(|_| panic!("ROM not found: {rel}"))
}

fn run_blargg(bytes: &[u8], timeout_frames: u32) -> String {
    let mut emu = gpuboy_core::Emulator::new(bytes.to_vec()).expect("load");
    let mut out = String::new();
    for _ in 0..timeout_frames {
        emu.step_frame();
        out.push_str(&String::from_utf8_lossy(&emu.take_serial_output()));
        if out.contains("Passed") || out.contains("Failed") {
            break;
        }
    }
    out
}

#[test]
fn cpu_instrs_01_special() {
    let out = run_blargg(&rom("blargg/cpu_instrs/individual/01-special.gb"), 600);
    assert!(out.contains("Passed"), "output: {out:?}");
}

#[test]
fn cpu_instrs_02_interrupts() {
    let out = run_blargg(&rom("blargg/cpu_instrs/individual/02-interrupts.gb"), 600);
    assert!(out.contains("Passed"), "output: {out:?}");
}

#[test]
fn cpu_instrs_03_op_sp_hl() {
    let out = run_blargg(&rom("blargg/cpu_instrs/individual/03-op sp,hl.gb"), 600);
    assert!(out.contains("Passed"), "output: {out:?}");
}

#[test]
fn cpu_instrs_04_op_r_imm() {
    let out = run_blargg(&rom("blargg/cpu_instrs/individual/04-op r,imm.gb"), 600);
    assert!(out.contains("Passed"), "output: {out:?}");
}

#[test]
fn cpu_instrs_05_op_rp() {
    let out = run_blargg(&rom("blargg/cpu_instrs/individual/05-op rp.gb"), 600);
    assert!(out.contains("Passed"), "output: {out:?}");
}

#[test]
fn cpu_instrs_06_ld_r_r() {
    let out = run_blargg(&rom("blargg/cpu_instrs/individual/06-ld r,r.gb"), 600);
    assert!(out.contains("Passed"), "output: {out:?}");
}

#[test]
fn cpu_instrs_07_jr_jp_call_ret_rst() {
    let out = run_blargg(
        &rom("blargg/cpu_instrs/individual/07-jr,jp,call,ret,rst.gb"),
        600,
    );
    assert!(out.contains("Passed"), "output: {out:?}");
}

#[test]
fn cpu_instrs_08_misc() {
    let out = run_blargg(&rom("blargg/cpu_instrs/individual/08-misc instrs.gb"), 600);
    assert!(out.contains("Passed"), "output: {out:?}");
}

#[test]
fn cpu_instrs_09_op_r_r() {
    let out = run_blargg(&rom("blargg/cpu_instrs/individual/09-op r,r.gb"), 600);
    assert!(out.contains("Passed"), "output: {out:?}");
}

#[test]
#[ignore] // CB-prefix bit ops on (HL) memory
fn cpu_instrs_10_bit_ops() {
    let out = run_blargg(&rom("blargg/cpu_instrs/individual/10-bit ops.gb"), 600);
    assert!(out.contains("Passed"), "output: {out:?}");
}

#[test]
#[ignore] // ALU ops using (HL) memory addressing
fn cpu_instrs_11_op_a_hl() {
    let out = run_blargg(&rom("blargg/cpu_instrs/individual/11-op a,(hl).gb"), 600);
    assert!(out.contains("Passed"), "output: {out:?}");
}

#[test]
#[ignore] // halt bug not implemented (PC stall when HALT with IME=0 and pending interrupt)
fn halt_bug() {
    let out = run_blargg(&rom("blargg/halt_bug.gb"), 300);
    assert!(out.contains("Passed"), "output: {out:?}");
}

#[test]
fn instr_timing() {
    let out = run_blargg(&rom("blargg/instr_timing/instr_timing.gb"), 300);
    assert!(out.contains("Passed"), "output: {out:?}");
}

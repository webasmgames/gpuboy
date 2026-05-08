# Phase 2: CPU + Timer + Interrupts + Serial Stub

## Overview

Implements the Sharp LR35902 CPU (all ~500 opcodes), the hardware timer (DIV/TIMA/TMA/TAC), the interrupt subsystem (IME, IF, IE, five vectors), and a zero-latency serial stub that captures bytes written via SB/SC. After this phase the emulator can fetch-decode-execute instructions, tick the timer, and dispatch interrupts — the foundation every subsequent phase builds on. There is no PPU yet, so no VBlank source; the emulator just runs the CPU hot for one frame's worth of cycles per `step_frame` call.

## Requirements

1. WHEN `step()` is called, THEN the CPU fetches the byte at PC, decodes it as an LR35902 opcode, updates all affected registers/flags/memory, advances PC by the instruction length, and returns the number of T-cycles consumed (4–24 depending on opcode and branch taken).

2. WHEN `step()` encounters an undefined opcode (0xD3, 0xDB, 0xDD, 0xE3, 0xE4, 0xEB, 0xEC, 0xED, 0xF4, 0xFC, 0xFD), THEN the CPU panics with a message identifying the opcode address.

3. WHEN `step()` executes EI, THEN IME does not activate until after the next instruction completes (one-instruction delay).

4. WHEN `step()` executes DI, THEN IME is cleared immediately with no delay.

5. WHEN `step()` is called with IME=true and (IF & IE & 0x1F) ≠ 0, THEN the CPU pushes PC to the stack, loads PC with the vector of the lowest-numbered set bit (bit 0 → 0x0040, bit 1 → 0x0048, bit 2 → 0x0050, bit 3 → 0x0058, bit 4 → 0x0060), clears that bit in IF, clears IME, and consumes 20 T-cycles.

6. WHEN HALT executes with IME=false and (IF & IE & 0x1F) ≠ 0 at that moment, THEN the HALT bug triggers: the CPU does not halt; on the next opcode fetch PC is not incremented (the byte is read twice).

7. WHEN HALT executes and the wakeup condition ((IF & IE & 0x1F) ≠ 0) is not yet true, THEN `step()` returns 4 T-cycles each call (stalling) until the condition becomes true, at which point the CPU resumes; if IME=false it resumes without dispatching an interrupt handler.

8. WHEN any value is written to 0xFF04 (DIV), THEN the internal 16-bit timer counter resets to 0, making the next DIV read return 0.

9. WHEN TAC bit 2 is set and the timer period elapses, THEN TIMA increments; when TIMA overflows from 0xFF, TMA is reloaded into TIMA and bit 2 of IF is set.

10. WHEN 0xFF02 (SC) is written with bit 7 set, THEN the byte currently in SB (0xFF01) is appended to the serial output buffer, SC bit 7 is cleared, and bit 3 of IF is set.

11. WHEN `step_frame()` is called, THEN the emulator runs `step()` in a loop until at least 70224 T-cycles have been consumed.

## Acceptance Criteria

- [ ] `cargo test -p gpuboy-core` passes all unit tests (CPU opcode helpers, timer, serial stub, integration)
- [ ] NOP round-trip: a ROM of all 0x00 bytes runs 70224 T-cycles without panic
- [ ] Serial integration: inline ROM that writes 'H', 'i' via SB/SC; after `step_frame()`, `take_serial_output()` returns `[0x48, 0x69]`
- [ ] Timer interrupt: set TIMA=0xFF, TMA=0x00, TAC=0x04 (enabled, 4096 Hz); run 1024 T-cycles; verify IF bit 2 is set and TIMA=0x00 (reloaded from TMA)
- [ ] EI delay: set IF=0x01, IE=0x01, execute EI then NOP; interrupt should dispatch after NOP completes (not during EI step)
- [ ] HALT wakeup: CPU stalls on HALT; after setting IF bit 0 externally, CPU resumes
- [ ] HALT bug: HALT with IME=false and IF & IE ≠ 0 → next byte read twice (PC stays put)
- [ ] No regressions from Phase 1 (bus WRAM/HRAM/echo tests still pass)

## Design

### Architecture

```
gpuboy-core/src/
  lib.rs        — Emulator struct (owns Cpu + Bus); step(), step_frame(), take_serial_output()
  cpu.rs        — Cpu struct; step(), step_cb(); all LR35902 opcodes
  timer.rs      — Timer struct; read(), write(), step() → u8 (overflow count)
  bus.rs        — Bus struct; updated with IO regs + Timer field + serial stub
  cartridge.rs  — unchanged
```

`Emulator` is the single entry point used by `gpuboy-wasm`. It ties Cpu and Bus together. Timer is owned by Bus (Bus delegates IO register reads/writes to it and calls `step_timer` after each CPU instruction).

### Data Structures

**Cpu**

```rust
pub struct Cpu {
    pub a: u8, pub f: u8,   // AF pair; F = ZNHC_0000
    pub b: u8, pub c: u8,
    pub d: u8, pub e: u8,
    pub h: u8, pub l: u8,
    pub sp: u16,
    pub pc: u16,
    pub ime: bool,          // interrupt master enable
    ime_pending: bool,      // set by EI; promoted to ime at end of next instruction
    halted: bool,
    halt_bug: bool,         // when true, next fetch does not increment PC
}
```

Flags live in F bits 7-4 (bits 3-0 are always 0):
- bit 7 Z — zero
- bit 6 N — subtract
- bit 5 H — half-carry
- bit 4 C — carry

Invariant: bits 3-0 of F are always 0. All flag setters (`set_zf` etc.) must preserve this: write flags only to bits 7-4 and leave bits 3-0 as zero. `set_af` must mask the lower nibble of the F byte before storing (`f = val & 0xF0`).

Post-boot initial state (matches hardware after boot ROM exits):

| Reg | Value | Reg | Value |
|-----|-------|-----|-------|
| A   | 0x01  | F   | 0xB0  |
| B   | 0x00  | C   | 0x13  |
| D   | 0x00  | E   | 0xD8  |
| H   | 0x01  | L   | 0x4D  |
| SP  | 0xFFFE | PC | 0x0100 |
| IME | false  |    |       |

**Timer**

```rust
pub struct Timer {
    counter: u16,   // internal counter; increments each T-cycle; DIV = (counter >> 8) as u8
    tima: u8,
    tma: u8,
    tac: u8,
}
```

`Timer::step(t_cycles: u32) -> u8` advances the timer by `t_cycles` T-cycles and returns the number of TIMA overflows that occurred (usually 0 or 1, but can be >1 at high frequencies). The caller adds that count to IF bit 2 (i.e., if result > 0, set_if_bit(2)). Writing any value to 0xFF04 resets `counter` to 0.

The implementation must loop **one T-cycle at a time** internally to correctly detect falling-edge transitions. Batching the addition loses the edge detection for multi-cycle instructions at high TIMA frequencies.

TIMA increment frequency by TAC bits 1-0:

| TAC[1:0] | Hz     | T-cycles per tick | Counter bit that triggers |
|----------|--------|-------------------|--------------------------|
| 00       | 4096   | 1024              | bit 9                    |
| 01       | 262144 | 16                | bit 3                    |
| 10       | 65536  | 64                | bit 5                    |
| 11       | 16384  | 256               | bit 7                    |

Falling-edge detection per T-cycle: `prev_bit = (counter >> bit) & 1`; increment counter; `new_bit = (counter >> bit) & 1`; if TAC bit 2 set and prev_bit=1 and new_bit=0: increment TIMA; if TIMA overflows (wraps 0xFF→0x00): reload from TMA, increment overflow count.

**Bus additions**

```rust
pub struct Bus {
    // existing:
    cartridge: Cartridge,
    wram: [u8; 0x2000],
    hram: [u8; 0x7F],
    // new:
    timer: Timer,
    sb: u8,                  // 0xFF01
    sc: u8,                  // 0xFF02
    interrupt_flags: u8,     // 0xFF0F — IF register (bits 4-0)
    ie: u8,                  // 0xFFFF — IE register (bits 4-0)
    serial_buf: Vec<u8>,
}
```

New Bus methods:

```rust
pub fn if_reg(&self) -> u8                  // returns interrupt_flags
pub fn ie(&self) -> u8
pub fn set_if_bit(&mut self, bit: u8)       // interrupt_flags |= 1 << bit
pub fn step_timer(&mut self, t_cycles: u32) // calls timer.step; if result > 0: set_if_bit(2)
pub fn take_serial_output(&mut self) -> Vec<u8>
```

Note: the field is named `interrupt_flags` to avoid a Rust footgun where `bus.if_reg` would be ambiguous between the field and the method. The public accessor is `if_reg()` which mirrors the hardware register name.

**Emulator**

```rust
pub struct Emulator {
    pub cpu: Cpu,
    pub bus: Bus,
}

impl Emulator {
    pub fn new(rom: Vec<u8>) -> Result<Self, String>
    pub fn step(&mut self) -> u32       // one CPU instruction + timer step; returns T-cycles
    pub fn step_frame(&mut self)        // loop step() until ≥ 70224 T-cycles
    pub fn take_serial_output(&mut self) -> Vec<u8>
}
```

### Key Decisions

**No boot ROM.** CPU initializes to post-boot register values (table above). This is standard emulator practice; the boot ROM is not needed for running games.

**EI delay via `ime_pending`.** EI sets `ime_pending = true`. `ime_pending` is promoted to `ime = true` at the *end* of the instruction that follows EI (i.e., at the end of the step *after* the EI step). This ensures EI + DI has no window, and interrupts can't fire during EI itself.

**Step order.** Within `Cpu::step`:
  1. If halted and wakeup condition false: return 4. If halted and wakeup condition true: `halted = false`, fall through to step 2 immediately (do not return) — this is how HALT wakes directly into an ISR when IME=true.
  2. If IME=true and (IF & IE & 0x1F) ≠ 0: dispatch interrupt (push PC, load vector, clear IME + IF bit), return 20.
  3. Fetch opcode: if `halt_bug` is true, read `bus.read(self.pc)` without calling `fetch_byte` (PC must not increment), then clear `halt_bug`. Otherwise call `fetch_byte` normally.
  4. Execute opcode; accumulate T-cycles.
  5. If `ime_pending`: set `ime = true`, clear `ime_pending`.
  6. Return T-cycles.

**Timer ownership inside Bus.** Timer registers live at 0xFF04–0xFF07; Bus already owns all memory-mapped IO. Putting Timer inside Bus keeps the address dispatch in one place and avoids a borrow-splitting problem in Emulator.

**Timer latency during HALT (known approximation).** When the CPU is halted, `Cpu::step` returns 4 T-cycles and `Emulator::step` calls `bus.step_timer` afterwards. This means the interrupt check inside HALT sees a stale IF: if a timer interrupt fires on the exact same step that wakes HALT, the CPU will see IF clear, stall another 4 T-cycles, then wake on the following step. This is a ≤4 T-cycle approximation error and is acceptable for Phase 2.

**Serial stub is zero-latency.** Real serial takes 8192 T-cycles (internal clock). We complete transfers immediately on SC write. This lets Blargg test ROMs emit output without blocking. Full serial timing is deferred indefinitely.

**TIMA overflow quirk deferred.** On hardware there is a 4-T-cycle window after TIMA overflows where reads return 0x00 before TMA is reloaded. Implementing this now adds complexity with negligible test-ROM benefit in Phase 2; defer to Phase 9.

**Undefined opcodes panic.** None of the 11 undefined opcodes appear in legal ROMs. Panicking surfaces bugs early rather than silently doing the wrong thing.

**CB register encoding.** The lower 3 bits of every CB opcode encode the operand register: 0=B, 1=C, 2=D, 3=E, 4=H, 5=L, 6=(HL), 7=A. Bits 5-3 encode the bit index for BIT/RES/SET. This makes all four CB groups (RLC/BIT/RES/SET) implementable as a pair of nested matches rather than 256 individual arms.

### Bus IO Map (additions)

| Address   | Register | Notes                                      |
|-----------|----------|--------------------------------------------|
| 0xFF01    | SB       | Serial data byte                           |
| 0xFF02    | SC       | Serial control; write bit 7 → stub transfer |
| 0xFF04    | DIV      | Read: counter>>8; write any: reset counter |
| 0xFF05    | TIMA     | Timer counter                              |
| 0xFF06    | TMA      | Timer modulo                               |
| 0xFF07    | TAC      | Timer control (bits 2-0 only; upper bits 1)|
| 0xFF0F    | IF       | Interrupt flags (bits 4-0)                 |
| 0xFFFF    | IE       | Interrupt enable (bits 4-0)                |

Reads of unmapped IO still return 0xFF. Writes to unmapped IO are silently dropped.

**OAM DMA (0xFF46):** Many ROMs write to 0xFF46 on startup. In Phase 2, writes to 0xFF46 are silently dropped (no CPU stall, no memory copy). Phase 3 will add real OAM DMA behavior when OAM memory exists.

## Tasks

- [x] 1. Create `crates/gpuboy-core/src/cpu.rs`: `Cpu` struct with all register fields and state booleans; flag helpers `zf/nf/hf/cf` (get) and `set_zf/set_nf/set_hf/set_cf`; register-pair helpers `af/bc/de/hl` (get, return u16) and `set_af/set_bc/set_de/set_hl` (set_af must mask lower nibble of F to 0); `fetch_byte(&mut self, bus: &mut Bus) -> u8` (reads bus at PC, increments PC); `fetch_word` (two fetch_byte calls, little-endian); `Cpu::new()` returning post-boot state. *(req 1)*

- [x] 2. Add `pub mod cpu;` to `lib.rs`. *(req 1)*

- [x] 3. Create `crates/gpuboy-core/src/timer.rs`: `Timer` struct with fields listed in Design; `Timer::new()`; `read(addr: u16) -> u8` (0xFF04→`(counter >> 8) as u8`, 0xFF05→tima, 0xFF06→tma, 0xFF07→`tac | 0xF8`, else 0xFF); `write(addr: u16, val: u8)` (0xFF04→`counter=0`, 0xFF05→`tima=val`, 0xFF06→`tma=val`, 0xFF07→`tac=val&0x07`); `step(t_cycles: u32) -> u8` — loop one T-cycle at a time using the falling-edge-on-bit method described in Design §Data Structures; accumulate overflow count and return it. *(req 8, 9)*

- [x] 4. Add `pub mod timer;` to `lib.rs`. *(req 8, 9)*

- [x] 5. Update `crates/gpuboy-core/src/bus.rs`: add `timer: Timer`, `sb: u8`, `sc: u8`, `interrupt_flags: u8`, `ie: u8`, `serial_buf: Vec<u8>` to `Bus`; update `Bus::new` to zero-initialize them; update `read` to handle: 0xFF01→sb, 0xFF02→sc, 0xFF04..=0xFF07→`timer.read(addr)`, 0xFF0F→`interrupt_flags`, 0xFFFF→ie; update `write` to handle: 0xFF01→`sb=val`, 0xFF02→serial stub (if `val&0x80 ≠ 0`: push sb to serial_buf, `sc=val&!0x80`, set IF bit 3; else `sc=val`), 0xFF04..=0xFF07→`timer.write(addr,val)`, 0xFF0F→`interrupt_flags=val&0x1F`, 0xFFFF→`ie=val&0x1F`; add `pub fn if_reg(&self)->u8` (returns `interrupt_flags`), `pub fn ie(&self)->u8`, `pub fn set_if_bit(&mut self, bit: u8)` (`interrupt_flags |= 1 << bit`), `pub fn step_timer(&mut self, t_cycles: u32)` (calls `timer.step(t_cycles)`; if result > 0: `set_if_bit(2)`), `pub fn take_serial_output(&mut self)->Vec<u8>` (drain serial_buf). *(req 8, 9, 10)*

- [x] 6. Implement `Cpu::step(&mut self, bus: &mut Bus) -> u32` in `cpu.rs` following exactly the step order in Design §Step order. HALT stall: return 4. HALT wakeup: set `halted=false`, fall through to interrupt check without returning. Interrupt dispatch: push PC high byte then low byte to stack (decrement SP by 1 before each push), set PC to the vector of the lowest-numbered set bit in `(if_reg() & ie() & 0x1F)`, clear that IF bit, set `ime=false`, return 20. Opcode fetch: if `halt_bug` is true, read opcode as `bus.read(self.pc)` (do NOT call `fetch_byte`; PC must not change), then set `halt_bug=false`; otherwise call `fetch_byte` normally. Delegate 0xCB prefix to `step_cb`. After executing the opcode, apply EI promotion: if `ime_pending { ime=true; ime_pending=false; }`. Return T-cycles. *(req 1, 3, 4, 5, 6, 7)*

- [x] 7a. Implement opcodes **0x00–0x3F** in the match block inside `Cpu::step`. Implementation notes for this range:
    - **0x00 NOP**: 4 cycles.
    - **0x01/0x11/0x21/0x31 LD rr,nn**: load 16-bit immediate into BC/DE/HL/SP; 12 cycles.
    - **0x02/0x12 LD (BC/DE),A**: store A to address in BC or DE; 8 cycles.
    - **0x03/0x13/0x23/0x33 INC rr**: 16-bit increment, no flags; 8 cycles.
    - **0x04/0x05/0x0C/0x0D/0x14/0x15/0x1C/0x1D/0x24/0x25/0x2C/0x2D/0x3C/0x3D INC/DEC r**: 8-bit inc/dec on B/C/D/E/H/L/A. INC: Z=(result==0), N=0, H=((old&0xF)==0xF). DEC: Z=(result==0), N=1, H=((old&0xF)==0x0). C unchanged. 4 cycles.
    - **0x34/0x35 INC/DEC (HL)**: same flag logic as INC/DEC r, but read/write via bus; 12 cycles.
    - **0x06/0x0E/0x16/0x1E/0x26/0x2E/0x3E LD r,n**: load immediate byte into B/C/D/E/H/L/A; 8 cycles.
    - **0x36 LD (HL),n**: load immediate byte to address in HL; 12 cycles.
    - **0x07 RLCA**: rotate A left; old bit 7 → C and bit 0; Z=0, N=0, H=0; 4 cycles.
    - **0x0F RRCA**: rotate A right; old bit 0 → C and bit 7; Z=0, N=0, H=0; 4 cycles.
    - **0x17 RLA**: rotate A left through carry; old C → bit 0, old bit 7 → C; Z=0, N=0, H=0; 4 cycles.
    - **0x1F RRA**: rotate A right through carry; old C → bit 7, old bit 0 → C; Z=0, N=0, H=0; 4 cycles.
    - **0x08 LD (a16),SP**: write SP low byte to (a16), high byte to (a16+1); 20 cycles.
    - **0x09/0x19/0x29/0x39 ADD HL,rr**: 16-bit add into HL; N=0, H=carry from bit 11, C=carry from bit 15; Z unchanged; 8 cycles.
    - **0x0A/0x1A LD A,(BC/DE)**: load from address in BC or DE into A; 8 cycles.
    - **0x0B/0x1B/0x2B/0x3B DEC rr**: 16-bit decrement, no flags; 8 cycles.
    - **0x10 STOP**: consume the mandatory padding byte (`fetch_byte` and discard), return 4 cycles. (No LCD or speed-switch in Phase 2.)
    - **0x18 JR e**: unconditional relative jump; signed 8-bit offset from byte after instruction; 12 cycles.
    - **0x20/0x28/0x30/0x38 JR cc,e**: conditional relative jump; not-taken: 8 cycles, taken: 12 cycles.
    - **0x22 LD (HL+),A**: store A to (HL), then HL++; 8 cycles.
    - **0x2A LD A,(HL+)**: load A from (HL), then HL++; 8 cycles.
    - **0x32 LD (HL-),A**: store A to (HL), then HL--; 8 cycles.
    - **0x3A LD A,(HL-)**: load A from (HL), then HL--; 8 cycles.
    - **0x27 DAA**: if N flag clear (last op was addition): if C set or `A > 0x99` → `A = A.wrapping_add(0x60)`, set C; if H set or `(A & 0x0F) > 0x09` → `A = A.wrapping_add(0x06)`. If N flag set: if C set → `A = A.wrapping_sub(0x60)`; if H set → `A = A.wrapping_sub(0x06)`. Always: Z=(A==0), H=0; C unchanged except where set above. 4 cycles.
    - **0x2F CPL**: A = !A; N=1, H=1; Z and C unchanged; 4 cycles.
    - **0x37 SCF**: C=1, N=0, H=0; Z unchanged; 4 cycles.
    - **0x3F CCF**: C=!C, N=0, H=0; Z unchanged; 4 cycles.
    - *(req 1)*

- [x] 7b. Implement opcodes **0x40–0x7F** in the match block inside `Cpu::step`. Implementation notes for this range:
    - This range is entirely LD r,r (or LD r,(HL) / LD (HL),r), plus HALT at 0x76.
    - Destination register in bits 5-3, source in bits 2-0, using the standard encoding: 0=B, 1=C, 2=D, 3=E, 4=H, 5=L, 6=(HL), 7=A. Rather than 63 individual arms, use a helper (or a small inline match) to read/write by register index; 4 cycles for r,r; 8 cycles when source or dest is (HL).
    - **0x76 HALT**: check HALT bug condition first. If IME=false and `(bus.if_reg() & bus.ie() & 0x1F) != 0`: set `halt_bug=true`, do NOT set `halted`. Otherwise set `halted=true`. Return 4 cycles.
    - *(req 1, 6, 7)*

- [x] 7c. Implement opcodes **0x80–0xBF** in the match block inside `Cpu::step`. Implementation notes for this range:
    - This range is ALU operations on register operands (or (HL)): ADD, ADC, SUB, SBC, AND, XOR, OR, CP. The low 3 bits encode the source register (same 0-7 encoding as 7b). 4 cycles for register source; 8 cycles for (HL) source.
    - **ADD A,r (0x80–0x87)**: result = A + r; C=carry from bit 7; H=carry from bit 3; Z=(result==0); N=0.
    - **ADC A,r (0x88–0x8F)**: result = A + r + cf; same flag logic as ADD.
    - **SUB r (0x90–0x97)**: result = A - r; C=(A<r); H=((A&0xF)<(r&0xF)); Z=(result==0); N=1.
    - **SBC A,r (0x98–0x9F)**: result = A - r - cf; same flag logic as SUB.
    - **AND r (0xA0–0xA7)**: A &= r; Z=(A==0); N=0; H=1; C=0.
    - **XOR r (0xA8–0xAF)**: A ^= r; Z=(A==0); N=0; H=0; C=0.
    - **OR r (0xB0–0xB7)**: A |= r; Z=(A==0); N=0; H=0; C=0.
    - **CP r (0xB8–0xBF)**: same as SUB but discard result; only flags are written.
    - *(req 1)*

- [x] 7d. Implement opcodes **0xC0–0xFF** in the match block inside `Cpu::step`. Implementation notes for this range:
    - **0xC0/0xC8/0xD0/0xD8 RET cc**: not-taken: 8 cycles; taken: pop PC from stack, 20 cycles.
    - **0xC1/0xD1/0xE1/0xF1 POP BC/DE/HL/AF**: pop two bytes from stack into register pair; 12 cycles. POP AF: use set_af (masks lower nibble of F to 0).
    - **0xC2/0xCA/0xD2/0xDA JP cc,nn**: not-taken: 12 cycles; taken: 16 cycles.
    - **0xC3 JP nn**: unconditional jump to 16-bit immediate; 16 cycles.
    - **0xC4/0xCC/0xD4/0xDC CALL cc,nn**: not-taken: 12 cycles; taken: push PC+3, jump to nn; 24 cycles.
    - **0xC5/0xD5/0xE5/0xF5 PUSH BC/DE/HL/AF**: push high byte then low byte (decrement SP before each); 16 cycles.
    - **0xC6/0xCE/0xD6/0xDE/0xE6/0xEE/0xF6/0xFE ALU A,n**: immediate-operand forms of ADD/ADC/SUB/SBC/AND/XOR/OR/CP; same flag logic as the 7c register forms; 8 cycles.
    - **0xC7/0xCF/0xD7/0xDF/0xE7/0xEF/0xF7/0xFF RST n**: push current PC, jump to 0x00/0x08/0x10/0x18/0x20/0x28/0x30/0x38; 16 cycles.
    - **0xC9 RET**: pop PC from stack; 16 cycles.
    - **0xCB**: fetch next byte, delegate to `step_cb`; total cycles = 4 + step_cb result.
    - **0xD9 RETI**: pop PC from stack, set IME=true immediately (no delay); 16 cycles.
    - **0xE0 LDH (a8),A**: address = 0xFF00 + fetch_byte; write A; 12 cycles.
    - **0xF0 LDH A,(a8)**: address = 0xFF00 + fetch_byte; read into A; 12 cycles.
    - **0xE2 LD (C),A**: address = 0xFF00 + C; write A; 8 cycles.
    - **0xF2 LD A,(C)**: address = 0xFF00 + C; read into A; 8 cycles.
    - **0xE8 ADD SP,e**: `e` is a signed 8-bit immediate; reinterpret as `e_u8 = e as u8`. Flags: Z=0, N=0; H=`((sp&0xF)+(e_u8&0xF))>0xF`; C=`((sp&0xFF)+e_u8 as u16)>0xFF`. Result: `sp.wrapping_add(e as i8 as i16 as u16)`. 16 cycles.
    - **0xF8 LD HL,SP+e**: same immediate and flag logic as ADD SP,e; result goes into HL, SP unchanged. 12 cycles.
    - **0xE9 JP HL**: PC = HL; 4 cycles.
    - **0xEA LD (a16),A**: write A to 16-bit immediate address; 16 cycles.
    - **0xFA LD A,(a16)**: read from 16-bit immediate address into A; 16 cycles.
    - **0xF3 DI**: IME=false, ime_pending=false immediately; 4 cycles.
    - **0xFB EI**: ime_pending=true (IME activates after next instruction); 4 cycles.
    - **0xF9 LD SP,HL**: SP = HL; 8 cycles.
    - **Undefined opcodes** (0xD3, 0xDB, 0xDD, 0xE3, 0xE4, 0xEB, 0xEC, 0xED, 0xF4, 0xFC, 0xFD): `panic!("undefined opcode {:#04X} at {:#06X}", opcode, self.pc.wrapping_sub(1))`.
    - All conditional (cc) opcodes: if not taken, return base cycle count; if taken, return extended cycle count.
    - *(req 1, 2, 3, 4)*

- [x] 8. Implement `Cpu::step_cb(&mut self, bus: &mut Bus) -> u32` in `cpu.rs`. Extract operand register from `opcode & 0x07` and bit index from `(opcode >> 3) & 0x07`. Use a helper `cb_reg_read(r: u8, bus: &Bus) -> u8` and `cb_reg_write(r: u8, val: u8, bus: &mut Bus)` for the 0-7 encoding. Match on `opcode >> 6` for group (0=shift/rotate, 1=BIT, 2=RES, 3=SET), then `(opcode >> 3) & 0x07` for operation within group 0 (RLC/RRC/RL/RR/SLA/SRA/SWAP/SRL). (HL) operand costs 4 extra T-cycles on reads and 4 more on writes. BIT sets Z=!(val & (1<<b)), N=0, H=1, does not write back. RES/SET write back; no flag changes. All shift/rotate ops set Z, clear N and H, set C from shifted-out bit; SWAP clears all flags except Z. *(req 1)*

- [ ] 9. Replace the body of `crates/gpuboy-core/src/lib.rs` with the `Emulator` struct: `pub struct Emulator { pub cpu: Cpu, pub bus: Bus }`. Implement `Emulator::new(rom: Vec<u8>) -> Result<Self, String>` (creates Bus via `Bus::new(Cartridge::load(rom)?)`  and `Cpu::new()`). Implement `pub fn step(&mut self) -> u32` (calls `cpu.step(&mut bus)`, then `bus.step_timer(t_cycles)`, returns t_cycles). Implement `pub fn step_frame(&mut self)` (accumulates t_cycles in a local until ≥ 70224). Implement `pub fn take_serial_output(&mut self) -> Vec<u8>` (delegates to `bus.take_serial_output()`). *(req 11)*

- [ ] 10. Update `crates/gpuboy-wasm/src/lib.rs`: add a `thread_local! { static EMULATOR: RefCell<Option<Emulator>> = RefCell::new(None); }`. Update `load_rom` to call `Emulator::new(data)` and store in the thread-local (log title on success, error string on failure). Add `#[wasm_bindgen] pub fn step_frame()` (borrows EMULATOR, calls step_frame if Some). Add `#[wasm_bindgen] pub fn take_serial_output() -> String` (borrows EMULATOR, calls take_serial_output, converts bytes to a UTF-8 lossy string, logs it to console, returns it). *(req 11)*

- [ ] 11. Add unit tests:
    - **`cpu.rs`**:
      - `nop_advances_pc`: build a tiny Bus with a NOP ROM; call `cpu.step(&mut bus)`; assert PC=0x0101, T-cycles=4.
      - `ld_a_imm`: opcode 0x3E 0x42; after step, A=0x42, PC=0x0102.
      - `add_a_flags_zero`: A=0x80, ADD A,A → A=0x00, Z=1, N=0, H=0, C=1.
      - `add_a_flags_half_carry`: A=0x0F, ADD A, (imm 0x01) → H=1.
      - `sub_borrow`: A=0x01, SUB 0x02 → A=0xFF, C=1, N=1.
      - `jp_nn`: opcode 0xC3 lo hi; PC = (hi<<8)|lo.
      - `call_ret`: CALL nn pushes PC+3, loads PC=nn; RET pops and jumps back.
      - `ei_delay`: set IF=0x01, IE=0x01; call step() for EI — assert no dispatch (returns 4 cycles, not 20); call step() for NOP — assert still no dispatch (ime_pending promoted at END of this step, interrupt check is at the TOP); call step() a third time — assert dispatch occurs (returns 20 cycles).
      - `halt_wakeup`: HALT with IF=IE=0x00; step returns 4; externally set IF bit 0, IE bit 0; next step resumes (PC advances).
      - `halt_bug_triggered`: HALT with IME=false, IF=0x01, IE=0x01; next step re-reads the same PC byte (PC does not advance).
    - **`timer.rs`**:
      - `div_increments_after_256`: step(256) → div reads as 1.
      - `div_reset`: write to 0xFF04; div reads as 0.
      - `tima_overflow_fires_interrupt`: set `tma=0x00`, TAC=0x04 (enabled, 4096 Hz); step(1024); TIMA overflows → returns 1, TIMA=0x00 (reloaded from TMA).
      - `timer_disabled`: TAC=0x00; step(10000); TIMA stays 0.
    - **`bus.rs`**:
      - `serial_stub_captures_byte`: write 0x48 to SB (0xFF01); write 0x81 to SC (0xFF02); assert take_serial_output()==[0x48] and IF bit 3 is set.
    - **`lib.rs`** (integration):
      - `serial_integration`: build a 32KB ROM (`vec![0u8; 0x8000]`) with `rom[0x0147] = 0x00` (ROM-only) and the following opcodes placed starting at offset 0x0100: `[0x3E, 0x48, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0x3E, 0x69, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0x18, 0xFE]` (LD A,'H'; LDH (0x01),A; LD A,0x81; LDH (0x02),A — sends 'H'; LD A,'i'; LDH (0x01),A; LD A,0x81; LDH (0x02),A — sends 'i'; JR -2 infinite loop). Call `step_frame()` once; assert `take_serial_output() == [0x48, 0x69]`.
    - *(all requirements)*

## Manual Testing

1. Run `cargo test -p gpuboy-core` and confirm all tests pass with no failures.
2. Run `cargo clippy -p gpuboy-core -- -D warnings` and confirm no warnings.
3. Build the WASM: `wasm-pack build crates/gpuboy-wasm --target web`.
4. Serve locally: `python -m http.server 8000`, open `http://localhost:8000/www/`.
5. Open the browser console. Confirm "gpuboy ready" appears on load.
6. Load a flat (MBC-0) ROM (e.g. a homebrew or the minimal test ROM used in unit tests). Confirm the title logs in console and no JS error appears.
7. After loading, call `step_frame()` from the browser console (`window.step_frame()` if exported) and confirm no panic / console error.
8. Confirm serial output: load a ROM that writes to SB/SC (or use the integration test ROM saved as a .gb file); after step_frame(), call `take_serial_output()` from the console and confirm expected bytes appear.

**Green light:** [ ]

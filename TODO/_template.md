# Phase N: [Name]

## Overview

What this phase achieves and why it matters for the emulator. One short paragraph.

## Requirements

User-facing behavior in EARS format. Each item is a WHEN/THEN statement describing observable behavior — no implementation details.

1. WHEN the user does X, THEN the system does Y.
2. WHEN condition Z, THEN outcome W.

## Acceptance Criteria

Specific, binary pass/fail tests derived from the requirements. Where applicable, include test ROM expectations. Josh uses this list during Manual Testing.

- [ ] Criterion A passes (e.g. blargg `cpu_instrs` test 01 prints "ok")
- [ ] Criterion B passes
- [ ] No regressions from prior phases

## Design

Technical decisions: module layout, data structures, key crates, algorithms. Explains *why* as well as *what*. Reference requirements by number where relevant.

### Architecture

Describe the module/crate structure and how pieces connect.

### Data Structures

Key types, their fields, and invariants.

### Key Decisions

Tradeoffs made and why. This is where alternatives get ruled out.

## Tasks

Numbered, atomic implementation steps. Each step should be independently verifiable and reference the requirement(s) it fulfills. Written so Claude can execute the full list in one session without asking questions.

- [ ] 1. [Task] *(req 1)*
- [ ] 2. [Task] *(req 2)*
- [ ] 3. [Task] *(req 1, 2)*

## Manual Testing

Step-by-step instructions Josh follows to verify the phase. Claude does not mark a phase complete — Josh does after running these steps.

1. Run `[command]` and confirm [observable outcome].
2. Load `[url or file]` and confirm [observable outcome].
3. Run test ROM `[name]` and confirm output matches expected.

**Green light:** [ ]

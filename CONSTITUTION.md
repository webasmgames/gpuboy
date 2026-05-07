# Constitution

Working agreement between Josh and Claude for the gpuboy project.

## What Claude Can Do

- Read and edit source files
- Create new source files
- Research documentation and reference material
- Run `cargo fmt` and `cargo clippy` **on demand only** (when Josh explicitly asks)

## What Claude Cannot Do

- Run **any** git command — `git add`, `git commit`, `git push`, `git status`, anything. No exceptions, ever.
- Exception: `git diff`, `git diff --staged`, and `git status` are allowed as read-only inspection commands.
- Run builds, tests, or compilation: `cargo build`, `cargo test`, `wasm-pack build`, `npm run`, `make`, or any equivalent.
- Run servers, watchers, or any long-lived process.

Josh handles all of the above manually. After Claude edits code, Josh reviews the diff, runs what's needed to verify, then stages, commits, and pushes.

## Phase Workflow

Phases follow a **spec-locked, one-shot** model:

1. Josh and Claude write the phase spec together in `TODO/phase-N.md`
2. Josh confirms the spec is locked
3. In a **fresh Claude session**, Josh says: *"Read TODO/phase-N.md and implement it"*
4. Claude implements the full phase without back-and-forth
5. Josh manually tests per the **Manual Testing** section of the spec
6. Josh gives the green light (or opens a follow-up session for fixes)

No implementation begins until the spec is locked. No new phase begins until the current phase has a green light.

## Spec Authorship

Josh and Claude write specs together. Claude drafts; Josh edits and approves. Once locked, the spec is the source of truth for that session.

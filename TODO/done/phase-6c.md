# Phase 6c: Visual Polish

## Overview

Four targeted UX fixes following Phase 6 browser testing: (1) zoom levels cause the canvas to outgrow the shell because the shell has a hard 700px cap — the shell should grow to fit its content instead; (2) the A/B button pair sits in a vertical column, missing the real DMG's diagonal slant; (3) the WebGPU unavailability error renders inside the screen bezel, hidden under the canvas, where users won't see it; (4) no branding is visible on the shell — the name "GpuBoy" and the version number should appear as decorative text on the DMG body.

**Prerequisites:** Phase 6 complete. No Rust changes — CSS and JS only.

## Requirements

1. WHEN the user selects any zoom level (2×, 3×, or 4×), THEN the GB shell expands horizontally to fully contain the canvas, bezel, D-pad, and A/B buttons with no overflow or clipping.

2. WHEN the page renders, THEN the A/B button pair is visually rotated ~20° clockwise (matching the DMG diagonal), with the B button upper-left and A button lower-right, and the letter labels remain upright and legible.

3. WHEN WebGPU initialisation fails, THEN an error message is displayed in a location that is visible without scrolling or interacting with any control. *(location TBD — see Options below; Josh picks before spec is locked)*

4. WHEN the page renders, THEN the text "GpuBoy" and the version number (e.g. "v0.1.0") are visible as decorative labels on the DMG shell body, styled to match the DMG aesthetic (small, light-coloured, italic or printed-style).

## Acceptance Criteria

- [ ] At 4× zoom the shell is wide enough to contain canvas + bezel + D-pad + A/B with no overflow. No horizontal scrollbar on a 1280px-wide viewport.
- [ ] At 2× zoom the shell shrinks accordingly; the layout stays centred and proportional.
- [ ] A/B button group is visibly angled ~20° clockwise. "A" and "B" labels are upright.
- [ ] WebGPU error message is immediately visible on page load (no scroll, no interaction required) when WebGPU is unavailable.
- [ ] "GpuBoy" and version number visible on the shell body; text is legible and styled consistently with the DMG aesthetic.
- [ ] No regressions: zoom, audio, play/pause, renderer toggle, responsive layouts all work.

## Design

### Architecture

Changes confined to `www/style.css` and `www/index.js`. No Rust, no HTML structure changes.

### Key Decisions

**Shell sizing — `width: fit-content` instead of `max-width: 700px`.**
The shell currently has `width: 100%; max-width: 700px`. At 4× the shell's natural content width is ~856px, blowing past the cap. Replacing with `width: fit-content` lets the shell hug its contents at every zoom level. The body is already `display: flex; justify-content: center`, so the shell stays centred. On viewports narrower than the shell (e.g., a laptop at 4×) a horizontal scrollbar appears — acceptable on desktop since the mobile/landscape breakpoints still handle small viewports correctly. Add `min-width: 400px` to prevent the shell from collapsing too far at 2× on very narrow windows.

**A/B slant — rotate the container, counter-rotate labels.**
`transform: rotate(20deg)` on `.gb-ab` tilts the whole button group. Since the buttons are circles, the shapes are unaffected. Add `transform: rotate(-20deg)` to the `span` label inside each button so text stays upright. The 20° matches the approximate angle on a real DMG.

**Error message location — options for Josh to pick:**

- **A — Toast overlay**: a small pill banner (`position: absolute`) in the top-right corner of `.gb-shell`, `z-index: 200`, semi-transparent dark background, auto-shown when error text is set. Does not affect layout. Disappears if Josh clicks an X.
- **B — Between toolbar and body**: `#error` moves out of the bezel and becomes a sibling of `.gb-toolbar` inside the shell (rendered between toolbar and `.gb-body`). Visible immediately, pushes body down. Error div is `display: none` by default and toggled via JS just as now.
- **C — In the toolbar**: a warning icon `⚠` appears in the toolbar row with a short inline text label ("WebGPU unavailable — using 2D canvas"). Compact, always visible without disrupting the body layout.

Recommendation: **B** — simplest to implement, no positioning hacks, visible without interaction.

### Tasks (pending error location decision)

- [x] 1. In `www/style.css`, replace `.gb-shell { width: 100%; max-width: 700px; }` with `width: fit-content; min-width: 400px; max-width: 100%;`. *(req 1)*

- [x] 2. In `www/style.css`, add `transform: rotate(20deg); align-items: center;` to `.gb-ab`. Wrap the text content of `#btn-a` and `#btn-b` in `<span>` tags in `index.html` and add `#btn-a span, #btn-b span { display: inline-block; transform: rotate(-20deg); }` to the CSS. *(req 2)*

- [x] 3. Move `#error` out of `.gb-bezel` in `index.html` — place it per the chosen option (A/B/C). Update `www/style.css` to style it in its new location. No JS changes needed if option B or C; option A requires adding a close button handler. *(req 3)*

- [x] 4. Add a `<div class="gb-brand">` element to the shell body (below the bezel, above the speaker area). Inside it: `<span class="gb-brand-name">GpuBoy</span>` and `<span class="gb-brand-version">v0.1.0</span>`. Style in CSS: small italic font, `--gb-text-dim` colour, positioned centrally below the screen. *(req 4)*

## Manual Testing

1. Serve locally. Open in Chrome at a viewport ≥ 1280px wide.
2. Open the hamburger menu. Click 4×. Confirm the shell expands to contain the canvas with no overflow or scrollbar.
3. Click 2×. Confirm the shell shrinks. Click 3× to restore. Confirm the shell centred at each zoom.
4. Inspect the A/B button group. Confirm it is visibly angled diagonally. Confirm "A" and "B" labels are upright.
5. Test on a browser or machine where WebGPU is unavailable (or temporarily break `init_renderer` in JS). Confirm the error message is visible immediately on load without scrolling.
6. Confirm "GpuBoy" and the version number are visible on the shell body in a legible, DMG-appropriate style.
7. Load a ROM. Confirm audio, zoom, play/pause, renderer toggle, and responsive layout all still work.

**Green light:** [x]

# Type Foundry — Day 2 Build Session (Coding Agent Prompt)
*Day 2 · v0.1-dev · Planned by Grok. UFO open/blend/save, a drawing window, and an MCP server on the existing command session.*
*Block 1 is the shared seam. Finish and commit it before anyone starts Block 2 or Block 3. After that commit, Block 2 and Block 3 may run in parallel worktrees. Block Z runs once, after both land.*

---

## CONTEXT

You are building **Type Foundry** — a local professional type kit. Design a font, blend two compatible faces, and drive every change from one Rust command API. House chrome, when you draw it, follows the Havelin v2 system surface. The glyph canvas stays neutral: bone ground, black fill, one handle color.

**Domain guardrail:** local font authoring only. Do not upload fonts, outlines, reference images, or prompts. Do not copy another foundry's outlines or Shift's source into this project.

**Tech stack**
- Rust edition 2024, `stable-x86_64-pc-windows-msvc`, serde 1 / serde_json 1.
- Workspace `T:\troy-freeform\TypeFoundry\App`. Members today: `foundry-core`, `foundry-api`, `foundry-cli`. Binary name `foundry` (package `foundry-cli`). Version 0.1.0.
- Cargo target dir is `C:/Users/Troy Havelin/AppData/Local/typefoundry-target` via `App/.cargo/config.toml`. The T: share strips execute permission. Never point a shortcut or a `cargo run` target at a binary on T:.
- Check, from the repo root: `powershell -ExecutionPolicy Bypass -File App/scripts/check.ps1`. That runs `cargo fmt --all -- --check`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings`. Run cargo from `App/`. PowerShell 5 has no `&&`. Chain with `;`.
- Remote: `git@github.com:thavelin/Type-Foundry.git`. This PC has no GitHub SSH key. Keep `origin` as the SSH URL. Push with the HTTPS helper in Block Z. Do not change global git config. Dubious ownership on the NAS path is handled per command with `-c safe.directory=%(prefix)///192.168.1.115/compute-work/troy-freeform/TypeFoundry`.

**Project root:** `T:\troy-freeform\TypeFoundry`
**Design reference:** `Design/README.md`. Chrome colors are the Frame Extractor set: PAGE `0x030303`, PANEL `0x090907`, RAISED `0x11110D`, HAIRLINE `0x302A1E`, HAIRLINE_STRONG `0x5B4A2D`, INK `0xDED9CE`, MUTED `0x9D988C`, BONE `0xF3EEE4`, FOCUS `0xD8FF00`, AMBER `0xE8B65A`, SIGNAL `0x8AE6A3`, ALERT `0xF07461`, INVERSE `0x030303`. On-curve handles FOCUS, off-curve handles AMBER, selected handle SIGNAL. Do not vendor Inter or IBM Plex. Use egui's default fonts.

**What already works (DO NOT break):**
- Day 1 commit `a10eacf` on `main`: `typefoundry.font` version 1, compatible-outline blend, `foundry` commands `new`, `info`, `check`, `blend`, `run`.
- `Session::execute` / `execute_line` in `App/crates/foundry-api/src/lib.rs` is the only mutation path. Blank lines and `#` comments are skipped. A leading UTF-8 BOM is stripped. Responses are `{ok, error?, data?}`. Check and blend failures include `data.issues` and `ok: false`.
- Blend: linear lerp. `t=0` is font A, `t=1` is font B, outside that range extrapolates. Result name is `{A} / {B} @ {t}` with `t` trimmed. Refuses NaN `t` and incompatible pairs (UPM, glyph names, unicode, contour count, closed, point count, PointKind, smooth). Glyph order follows A. Missing glyphs are reported both ways.
- A fresh clone of `main` is the Day 1 engine only. This workspace may also have uncommitted UFO edits: `norad = "0.18"` in `foundry-core`, `mod ufo;` in `lib.rs`, and `FoundryError::MissingPoint` plus `Ufo(String)`. `ufo.rs` may be missing, and `Font::load` / `save` may still be JSON-only. If those edits are present, keep them and finish them in Block 1. If the tree is clean, add them in Block 1. Do not reset `a10eacf`.

**Helpers you'll reuse:** `Font::load`, `Font::save`, `Font::insert_glyph`, `Font::glyph` / `glyph_mut`, `Session::execute`, `blend_fonts`, `compatibility`. `foundry-cli` does not depend on `foundry-core`. It only talks to `foundry-api`.

**Quality bar:** `powershell -ExecutionPolicy Bypass -File App/scripts/check.ps1` clean at every checkpoint. **Real geometry only** — tests build synthetic fonts with norad. Never commit a commercial or third-party font. **Domain guardrail** above.

**Strategic focus:** Troy picked slices 1, 2, and 4: UFO open/blend/save, a desktop drawing window that shows outlines and moves points, and an MCP server so chat can drive the font. Generation from a prompt or an image stays frozen. `put_glyph` remains the seam for a later generator. In-app model calls, when they exist, use SpaceXAI. Do not add that client today.

---

## How to split the agents

| Agent | Owns | Starts when |
| --- | --- | --- |
| Seam | Block 0 and Block 1 | Now |
| Window | Block 2 only: `App/crates/foundry-app/**`, `Design/README.md`, one new line in `App/Cargo.toml` members | After Block 1 is committed |
| MCP | Block 3 only: `App/crates/foundry-mcp/**`, the `foundry mcp` subcommand in `foundry-cli`, one new line in `App/Cargo.toml` members, MCP section in `documents/api.md` | After Block 1 is committed |
| Closer | Block Z | After Blocks 2 and 3 are both committed |

Window and MCP agents call `Session`. They do not add font mutations. They do not edit `foundry-core`. They do not edit `Agent/CONTEXT.md` or Notion. The closer does that.

Parallel agents add only their own workspace member. Binary names are fixed: CLI stays `foundry`, window binary is `typefoundry`, MCP binary is `foundry-mcp`.

---

## ✅ BLOCK 0 — Inspect before you build (no code yet)

1. From `T:\troy-freeform\TypeFoundry`, run git status with the one-shot safe.directory flag. A clean tree at `a10eacf` is the GitHub state. An uncommitted UFO stub may exist in this workspace. Do not reset it. Do not amend `a10eacf`.
2. Read `App/crates/foundry-core/src/font.rs`, `error.rs`, `lib.rs`, `blend.rs`, `App/crates/foundry-api/src/lib.rs`, `App/crates/foundry-cli/src/args.rs`, `documents/api.md`, `Agent/CONTEXT.md`, `Design/README.md`.
3. Report the current `Font` load/save, the `Command` enum, and whether the UFO stub is present. If this prompt conflicts with the code, follow the code and say so.
4. Do not write feature code in this block. If `mod ufo` is present and `ufo.rs` is not, `cargo check -p foundry-core` fails. That failure is expected. Do not "fix" it by deleting `mod ufo`.

**CHECKPOINT 0:** shapes reported, no feature code beyond a stub already on disk, `a10eacf` untouched.

---

## ✅ BLOCK 1 — UFO through the existing commands, plus move_point

**Goal:** `foundry open`, `save`, `check`, and `blend` accept a `.ufo` directory the same way they accept JSON. A session can move one point. No second mutation API.

**Build:**
1. Add `App/crates/foundry-core/src/ufo.rs` with `is_ufo_path`, `load_ufo`, `save_ufo`. A path is UFO when its extension is `ufo` (this works for a directory).
2. `Font::load` / `Font::save` dispatch on that check. JSON behavior stays. `save` still calls `validate` first.
3. Add `Font::move_point(&mut self, name: &str, contour: usize, index: usize, x: f64, y: f64) -> Result<(), FoundryError>`. Reject non-finite coordinates with `NonFinite`. Missing glyph is `MissingGlyph`. A bad contour or point index is `MissingPoint`.
4. Import with norad 0.18: `Font::load`, `default_layer()`, `Layer::iter`. Sort glyph names before insert so order is stable. `Glyph::name().as_str()` is the name. `width` is the advance. First codepoint, if any, is `unicode` (`char` to `u32`). Extra codepoints are dropped. Anchors, guidelines, kerning, groups, and font lib are ignored.
5. Refuse a glyph that has `components` or an `image`. `FoundryError::Ufo` names the glyph and says components and images are not imported. Do not drop them silently.
6. Contour import: `Contour::is_closed()`. A leading `PointType::Move` means the contour is open; keep that point as `PointKind::On`. `OffCurve` is `Off`. `Line`, `Curve`, and `QCurve` are `On`. Copy `smooth` onto on-points. Off-points store `smooth: false`.
7. Contour export rebuilds point types. Off-points are `OffCurve`. The first point of an open contour is `Move`. Every other on-point looks at the off-points since the previous on-point, wrapping when the contour is closed and this is the first on-point: 0 offs → `Line`, 1 → `QCurve`, 2 → `Curve`. Any other count is `FoundryError::Ufo`. Build points with `ContourPoint::new(x, y, typ, smooth, None, None)` and `Contour::new(points, None)`.
8. Font info: `family_name` and `style_name` become the font name (`"{family} {style}"` when both exist, family alone when style is missing, the `.ufo` directory stem when both are missing). `units_per_em` is `NonNegativeIntegerOrFloat` (`as_f64` on read, `From<u32>` on write) and must fall in 16..=16384. `ascender` is `Option<f64>`. `cap_height`, `x_height`, and `descender` are `IntegerOrFloat` (a type alias for `f64`). Missing vertical metrics use the `Font::new` defaults for that UPM. Baseline is `0`.
9. `save_ufo` writes `norad::Font::new()`, fills `font_info`, inserts glyphs on `default_layer_mut()` with `insert_glyph`, then `save`. Map norad load/write errors into `FoundryError::Ufo` or `Io` with the norad message.
10. Add `Command::MovePoint { name, contour, point, x, y }` to the session (`point` is the index). It calls `Font::move_point` on the open font. `NoFont` when nothing is open. Return `{name, contour, point, x, y}`.
11. Tests in `foundry-core`: write two synthetic UFOs with norad in a temp dir (a square and the same square shifted), load both, blend at `t=0.5`, save UFO, load again, assert the midpoint advance and the first point. One cubic contour (two offs) and one open contour round-trip. A glyph with a component fails and names the glyph. Existing JSON tests still pass.
12. One `foundry-api` test: open a tiny UFO, `MovePoint`, save JSON, reload, assert the coordinate.
13. Update `documents/api.md`: `open` / `save` / `check` / `blend` accept `.ufo` directories and `.json` files. Document `move_point`. Document what UFO import ignores and what it refuses.

**Guardrails:** Detect UFO inside `Font::load` / `save`. Do not add `open_ufo` or a second blend. Do not import a real font file from disk as test data. `norad` is the only new dependency in this block.

**CHECKPOINT 1:**
- [ ] `powershell -ExecutionPolicy Bypass -File App/scripts/check.ps1` clean
- [ ] A temp UFO written by the test blends at 0.5 and the reloaded UFO shows the midpoint
- [ ] `foundry info` on a JSON file still prints name, UPM, and glyphs
- [ ] A component glyph returns `ok: false` and does not replace the open font

Commit: `git commit` with message `Open, blend, and save UFO through the command session.` Do not amend `a10eacf`.

---

## ✅ BLOCK 2 — Drawing window (only after Block 1 is committed)

**Goal:** A desktop window opens a JSON or UFO font, draws the selected glyph's outlines, and moves a point by dragging. The drag goes through `Command::MovePoint`.

**Build:**
1. New crate `App/crates/foundry-app`. Library holds viewport, outline flattening, and hit testing. `src/main.rs` is the eframe window. Add the member to `App/Cargo.toml` only.
2. Dependencies: `eframe = "0.36"` with `default-features = false` and features `default_fonts`, `glow`, `persistence`. `rfd = "0.15"`. `foundry-api` and `serde_json` from the workspace. Renderer is `Renderer::Glow`. Launch with `eframe::run_native`.
3. Binary name `typefoundry`, window title `Type Foundry`.
4. Chrome uses the hex colors in Context. Canvas clear is BONE. Filled outlines are black. Handles use FOCUS, AMBER, and SIGNAL as specified. Fit the glyph in the canvas. Scroll zooms. Dragging a handle writes absolute font coordinates via `Session::execute(Command::MovePoint ...)`.
5. Glyph list, open, and save. Open and save use the session (`Command::Open`, `Command::Save`) so UFO and JSON both work. File dialogs may start on the last folder. Remember the path in the window, not in `Session`.
6. Unit tests for fit-to-view, hit testing (on-point wins over a nearby off-point), and flattening a cubic (two offs) and a quadratic (one off) to a polyline. No window in those tests.
7. On first launch, create `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Type Foundry.lnk` pointing at `C:\Users\Troy Havelin\AppData\Local\typefoundry-target\release\typefoundry.exe`. Skip it when the shortcut already exists. Do not create it from `cargo test`.
8. Update `Design/README.md` to describe the window that now exists.

**Guardrails:** The window never writes `Font` fields itself. No browser. No prompt or image generation. No new color system.

**CHECKPOINT 2:**
- [ ] `powershell -ExecutionPolicy Bypass -File App/scripts/check.ps1` clean
- [ ] Hit-test and flatten tests pass
- [ ] From `App/`: `cargo run -p foundry-app --release`. The process starts, the window title is `Type Foundry`, then you stop the process
- [ ] The Start menu shortcut exists and its target is the C: `typefoundry.exe`

Commit: `Add a drawing window that moves points through the command session.`

---

## ✅ BLOCK 3 — MCP server (only after Block 1 is committed)

**Goal:** A stdio MCP server exposes the same session so a chat client can create, open, inspect, move a point, check, blend, and save. Generation tools are not included.

**Build:**
1. New crate `App/crates/foundry-mcp`. Hand-roll a small JSON-RPC stdio server. Do not depend on `rmcp`, tokio, or a macro crate. Add the workspace member only.
2. Binary `foundry-mcp`. Also add `foundry mcp` to `foundry-cli`, which runs the same server. `foundry-cli` may depend on `foundry-mcp`. It still must not depend on `foundry-core`.
3. Log to stderr only. Stdout is the protocol stream.
4. Messages: `initialize` echoes `protocolVersion`, sets `capabilities.tools`, and returns `serverInfo` name `typefoundry` version `0.1.0`. `notifications/initialized` returns nothing. `tools/list`, `tools/call`, and `ping` are supported. `id` may be a number or a string. A missing `id` is a notification and returns no response.
5. Tool result shape: `{content:[{type:"text", text}], isError}`. Tools call `Session::execute`. They do not touch `Font` directly.
6. Tools: `font_create {name, upm?}`, `font_open {path}`, `font_save {path?}`, `font_info`, `font_glyphs`, `glyph_get {name}`, `point_move {name, contour, point, x, y}`, `font_check {a, b}`, `font_blend {a, b, t?, out}`. `font_save` uses the path argument, or the last path this MCP process opened or saved. That memory lives in the MCP wrapper, not in `Session`.
7. Unit-test `handle_message` without a socket: initialize, a notification returns `None`, `tools/list` includes `point_move`, `point_move` on an open font changes the point, a bad path sets `isError` true, stdout of the test is empty of logs.
8. Document, in `documents/api.md`, the binary, `foundry mcp`, and a Claude / Cursor stdio config snippet that launches `foundry-mcp` with no arguments. State that tools never upload the font.

**Guardrails:** One session. No prompt tool, no image tool, no model client. Paths in examples use forward slashes.

**CHECKPOINT 3:**
- [ ] `powershell -ExecutionPolicy Bypass -File App/scripts/check.ps1` clean
- [ ] `handle_message` tests pass
- [ ] `foundry mcp` is in `foundry --help`
- [ ] A one-shot stdin initialize request prints one JSON object and nothing else on stdout

Commit: `Add an MCP server over the command session.`

---

## ✅ BLOCK Z — Verify, record, and push

Do this once, after Blocks 1–3 are committed on the same branch.

1. From the repo root: `powershell -ExecutionPolicy Bypass -File App/scripts/check.ps1`. It must pass.
2. Update `Agent/CONTEXT.md` session log for 2026-10-04 Day 2: what shipped, the check command result, and the next slice (generation stays frozen; say what is actually next). Fix the stale "Next: UFO" line from Day 1.
3. Update `README.md` so the command list includes UFO paths, the window, and `foundry mcp`.
4. Update the Notion hub https://app.notion.com/p/3ef627d6cfdc81e4a936e4f714b7aff0 Current State, Next 5 Actions, Build Log, and Blockers. Leave Priority at Next. Do not edit the Owner's Notes section.
5. Commit: `Record the UFO, window, and MCP slice.`
6. Push. Do not change global git config. Do not use `git push origin` (SSH key is denied). From the repo, with the safe.directory flag on the same command:

```text
git -c safe.directory=%(prefix)///192.168.1.115/compute-work/troy-freeform/TypeFoundry -c credential.helper= -c "credential.helper=!gh auth git-credential" push https://github.com/thavelin/Type-Foundry.git HEAD:main
```

7. Reply with: each checkpoint result, every commit SHA, whether the window smoke and the MCP initialize smoke passed, and anything Troy still has to click himself.

**Definition of done:** UFO open/blend/save, the `typefoundry` window, and `foundry mcp` are on `main` at v0.1-dev. `check.ps1` is clean. The hub matches the code. Owner's Notes is unchanged.

---

## STYLE RULES

- Chrome uses only the named hex colors. Canvas is BONE, fill is black, handles are FOCUS / AMBER / SIGNAL.
- Reuse `Session::execute`. Do not add a parallel font store.
- Match the surrounding Rust: explicit `Result`, no unwrap in library paths, clippy `-D warnings`.
- JSON Windows paths in docs use forward slashes.

## DO NOT

- Do not amend `a10eacf`.
- Do not upload fonts, outlines, reference images, or prompts.
- Do not copy Shift source, vendor Shift, or depend on Shift crates.
- Do not add prompt or image generation, a SpaceXAI client, or a TTF/OTF exporter.
- Do not add a second way to mutate a font.
- Do not edit Owner's Notes.
- Do not change global git config, the execution policy, or `App/.cargo/config.toml`.
- Do not create the Start menu shortcut except when the window binary actually launches.
- Do not leave a checkpoint with `check.ps1` failing.
- Do not commit `_MAP.md` or anything under `documents/Keys/`.

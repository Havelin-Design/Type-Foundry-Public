# Type Foundry

A local professional type kit. Design a font, blend two compatible faces into a new one, generate starting outlines from a prompt or an image, export real fonts, and drive all of that from plugins and from an agent.

## Folder map

- `App/` — Rust workspace. `crates/foundry-core` is the font model, blend, UFO exchange, and TrueType export. `crates/foundry-api` is the command session. `crates/foundry-cli` is the `foundry` binary. `crates/foundry-app` is the `typefoundry` drawing window. `crates/foundry-mcp` is the `foundry-mcp` stdio MCP server.
- `Agent/CONTEXT.md` — this file.
- `Design/` — the window as built, its chrome tokens, and editor direction.
- `documents/api.md` — the command contract.
- `documents/shift-reference.md` — what we take from Shift, and what we do not copy.
- `documents/Keys/` — credentials, gitignored. None yet.
- `Fonts/` — Roboto English and Roboto Cyrillic, as Type Foundry JSON. See `Fonts/NOTICE.md`.

## Collaboration

PM notes live here. Product code lives in `App/`. Plugins, the CLI, and agents share `foundry-api`. Do not add a second way to change a font.

## Project facts

- Path: `T:\troy-freeform\TypeFoundry`
- Remote: `git@github.com:thavelin/Type-Foundry.git` — https://github.com/thavelin/Type-Foundry
- Stack: Rust edition 2024, stable MSVC, serde, norad 0.18 (UFO), eframe 0.36 on glow plus rfd 0.15 (window). The MCP server is hand-rolled JSON-RPC with no async runtime.
- Version: `v0.1-dev`. Build day 2.
- Run from `App/`: `cargo run -p foundry-cli -- --help`. Window: `cargo run -p foundry-app --release`. MCP: `cargo run -p foundry-mcp` or `foundry mcp`.
- Check: `powershell -ExecutionPolicy Bypass -File App/scripts/check.ps1` from the repo root. This machine's execution policy rejects unsigned scripts.
- Build output: `C:\Users\Troy Havelin\AppData\Local\typefoundry-target` via `App/.cargo/config.toml`. This share creates programs without execute permission, so the target directory stays on `C:`.
- Hub: https://app.notion.com/p/3ef627d6cfdc81e4a936e4f714b7aff0 — Projects database, priority Next.
- Agent workspace: https://app.notion.com/p/3ef627d6cfdc8187905bfeaf3fed4d0e
- Design page: https://app.notion.com/p/3ef627d6cfdc818abbe0e6be4187020d
- Owner's Notes: the `Owner's Notes` section at the bottom of that hub page. Agents do not edit it.
- Domain guardrail: local font authoring only. Do not upload fonts, outlines, reference images, or prompts. Do not copy another foundry's outlines or Shift's source into this project.

## How a font is stored

The working file is JSON, `format` `typefoundry.font`, `version` 1. A glyph is contours of `on` and `off` points, an advance, and an optional unicode. `open`, `save`, `check`, and `blend` also read and write a `.ufo` directory through the same commands. `open` also reads a Three.js typeface JSON file, a webfontjson file, `.ttf`, `.otf`, `.ttc`, and `.otc`. WOFF, WOFF2, and Embedded OpenType are refused. Saving over `.otf`, `.ttc`, `.otc`, `.woff`, `.woff2`, or `.eot` is refused. UFO import keeps the default layer, sorts glyph names, and keeps the first Unicode value. Anchors, guidelines, kerning, groups, and lib data are ignored. Components, images, and implied-on qcurves are refused. `save` to a `.ttf` path writes an installable TrueType file: cubics become quadratics, open contours are closed with a straight edge, and Unicode outside the Basic Multilingual Plane is refused.

## How blend works

`t` 0 is the first font, `t` 1 is the second, `t` 0.5 is the midpoint. Values outside that range extrapolate. The blend is refused unless both fonts share units per em, glyph names, unicode, contour counts, closed flags, point counts, point kinds, and smooth flags. That is the variable-font master rule. Unrelated typefaces are a later matching problem, not a silent morph.

## Strategic focus

The product is a full type bench: edit outlines, blend faces, generate from a prompt or an image, export installable fonts, and let plugins and agents use one API.

Frozen for now:

- No account, sync, store, or upload.
- No fork of Shift and no dependency on Shift crates. Shift is the architecture reference. Its public app already draws, interpolates masters, and exports variable TrueType. Scripting and an AI API are still on its future list. That gap is ours.
- JSON `typefoundry.font` stays the working document. UFO is an exchange path on the same load and save calls, not a second editor model.
- Generation calls a provider through `put_glyph`. It does not live inside the geometry crate. In-app model calls use SpaceXAI when that day comes.

## Decisions

- Rust owns the font. The `typefoundry` window is a client of `foundry-api`. A drag is a `move_point` command.
- The JSON command stream is the plugin and agent API. `foundry-mcp` wraps it for chat clients. It does not get its own font mutations.
- Blend refuses incompatible outlines and reports why.
- House UI uses the Havelin v2 system surface for chrome. The glyph canvas stays neutral: BONE ground, black fill, FOCUS / AMBER / SIGNAL handles.
- Cargo target directory stays on `C:`.

## Session log

### 2026-10-04 — Editor UI (cloud)

- Focus: turn the bare window into an editor. Branch `claude/editor-ui`.
- Engine: 20 new session commands (point, contour, glyph, and font edits, `transform` with a per-glyph `anchor`, `round_coordinates`, `index`, `undo`, `redo`, `checkpoint`, `history`). Edits check their input before changing anything. Undo keeps 200 steps, and drags coalesce into one step.
- Window: a menu bar, toolbar, overview grid of cached thumbnails, select and pen tools, box selection, Alt-click to split a segment, an inspector, an Effects dialog with a live preview, a text preview strip, a settings window, and keyboard shortcuts. Split into `app`, `canvas`, `grid`, `panels`, `effects`, and `settings` modules.
- Fixed on the way: the typeface.js importer read `q` and `b` control points before the end point, which scrambled every curve. The two `Fonts/Roboto-*.json` subsets were regenerated from the json-fonts source.
- Validation: fmt, `cargo test --workspace`, and clippy `-D warnings` passed in the Linux container. Smoke under Xvfb on Roboto English: overview, editor, box select plus nudge (one undo step), undo, a 12-degree slant on all 95 glyphs from Effects, a new glyph drawn with the pen and closed, a point added by Alt-click and dragged, smooth toggled, and the result saved.
- Next: the MCP server still lists its original 9 tools. Add the new edit commands there. Then component support, kerning, and a Bold effect (path offsetting).

### 2026-10-04 — Binary font import (cloud)

- Focus: open more formats through the same `Font::load`. Branch `claude/font-import`.
- Shipped `.ttf`, `.otf` (CFF or CFF2), and face 0 of `.ttc` / `.otc` through `open`, `check`, and `blend`, read with `ttf-parser` (now a normal dependency). Composites are decomposed. Implied on-curve points become real. Variable fonts give their default instance. WOFF and WOFF2 are refused by name. Saving to a binary extension other than `.ttf` is refused. TrueType export now writes `post` format 2, so names survive a round trip.
- Validation: fmt, `cargo test --workspace` (34 tests), and clippy `-D warnings` passed in the Linux container. Tests build their own fonts, including a hand-assembled CFF font. No third-party font is committed. A smoke read 58 local fonts with no failures, including a 45,000-glyph CJK `.ttc` in about 0.34 s.
- Next: see `documents/font-formats.md`. WOFF 1 and choosing a collection face are the cheap steps. Reading variable-font instances as blend masters is the valuable one.

### 2026-10-04 — Roboto English, Roboto Cyrillic, and web font import

- Focus: bring in the two sample faces Troy asked for, and open the JSON font formats those repos use.
- Shipped `Fonts/Roboto-English.json` (U+0020–U+007E, 95 glyphs) and `Fonts/Roboto-Cyrillic.json` (U+0400–U+04FF, 255 glyphs). Both are Roboto Regular read from https://github.com/7dir/json-fonts `fonts/cyrillic/roboto/Roboto_Regular.json`. That repo has no separate English file. The other scripts in the source file were left out. License: `Fonts/NOTICE.md` and `Fonts/LICENSE-APACHE.txt`.
- `open` now reads typeface JSON (`m` `l` `q` `b` `z`), webfontjson (`css` with a base64 `@font-face`, including the `callback({...})` wrapper), and `.ttf`, `.otf`, `.woff`, `.woff2`. A multi-face web font imports the regular face. `.eot` is refused. Save will not overwrite `.otf`, `.woff`, or `.woff2`.
- The Myriad Pro files in https://github.com/ahume/webfontjson were not copied. That repo is the JSON wrapper, not a font library.
- Validation: `powershell -ExecutionPolicy Bypass -File App/scripts/check.ps1` passed. 34 tests, plus one ignored regen test, clippy clean. The Roboto files load as 95 English glyphs and 255 Cyrillic glyphs. The Open dialog filters were not clicked.
- Next: TrueType in the window Save As dialog, then pen and select tools. Generation stays frozen.

### 2026-10-04 — Windows check of the merged window and MCP

- Focus: PR #1 is on `main`. Confirm the cloud slice on this PC.
- `main` is `b7eef53`, the merge of `37719dc` onto `dd0ae32`. The local checkout was fast-forwarded to that commit. `foundry-core` was not changed by the window or MCP commits.
- Validation: `powershell -ExecutionPolicy Bypass -File App/scripts/check.ps1` passed. 28 tests (4 api, 8 app, 1 cli, 9 core, 6 mcp), clippy clean.
- The release `typefoundry.exe` launched with title `Type Foundry`. The Start menu shortcut `Type Foundry.lnk` was created on first launch and points at `C:\Users\Troy Havelin\AppData\Local\typefoundry-target\release\typefoundry.exe`. The process was then closed.
- Not done: Save As offers `.json` and `.ufo` only, so a `.ttf` was not saved from the window. `save` and `font_save` already write `.ttf`.
- Next: add TrueType to the window Save As dialog and proof one file. Then pen and select tools as session commands. Generation stays frozen.

### 2026-10-04 — Day 2 (cloud): drawing window and MCP server

- Focus: Blocks 2 and 3 of `documents/day-2-agent-prompt.md`. Block 1 was already on `main` (`82f747c`), so it was not rewritten. The cloud agent first built its own Block 1, found `main` had moved, and replayed only the window and MCP commits onto `dd0ae32`.
- Shipped `foundry-mcp` plus `foundry mcp` (`454055d`): 9 tools over `Session::execute`, stdout is protocol only, logs on stderr, `font_save` remembers the last path in the wrapper. Shipped the `typefoundry` window (`8a3317e`): glyph list, fitted canvas, scroll zoom, drag a handle to send `move_point`, open and save JSON or UFO, Start menu shortcut on first Windows launch.
- Validation: built in a Linux cloud container, so the check ran as its three commands (`cargo fmt --all -- --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`) with `RUSTUP_TOOLCHAIN=stable` and a local `CARGO_TARGET_DIR`. `rust-toolchain.toml` pins MSVC and there is no PowerShell there. All passed: 28 tests. `foundry-app` also passed clippy for `x86_64-pc-windows-msvc`, which covers the Windows-only shortcut code.
- Window smoke under Xvfb: the release `typefoundry` started with title `Type Foundry` and drew a UFO. Dragging a point and clicking Save wrote the new coordinate to the UFO. The Start menu shortcut was not exercised because it only runs on Windows.
- MCP smoke: one `initialize` line on stdin printed exactly one JSON object on stdout. `foundry mcp` is in `foundry --help`.
- Linux note: `foundry-app` turns on eframe's `x11` and `wayland` features under `cfg(target_os = "linux")` only. The Windows dependency set is the one the plan named.
- Next: run `check.ps1` and launch the window once on Troy's PC so the shortcut exists. Then proof a `.ttf` saved from the window. After that, pen and select tools (add and delete points, toggle smooth, undo) as new session commands, so plugins and MCP get them too. Generation stays frozen.


### 2026-10-04 — TrueType export

- Focus: the step after UFO, while the drawing window and MCP server are Claude's cloud slice from `documents/day-2-agent-prompt.md`.
- Shipped `Font::save` for a `.ttf` path, so `save` on the command stream writes an installable TrueType file. `.notdef` is glyph 0. Tests parse the file with `ttf-parser`.
- Validation: `powershell -ExecutionPolicy Bypass -File App/scripts/check.ps1` passed. 14 tests, clippy clean.
- Deferred: the window and MCP stay with the cloud agent. Generation stays frozen. OTF/CFF and non-BMP cmap are later.
- Next: land the cloud window and MCP on `main`, then proof a saved `.ttf` from the window.

### 2026-10-04 — Day 1

- Focus: stand up the project and the blend engine, using `thavelin/Type-Foundry` as the remote. Shift is the reference, not the codebase.
- Shipped the workspace, the `typefoundry.font` document, compatible-outline blending, and the `foundry` command stream (`new`, `info`, `check`, `blend`, `run`).
- Validation: `cargo test --workspace` 7/7 passed. `cargo clippy --workspace --all-targets -- -D warnings` passed. `cargo fmt --all -- --check` passed. CLI smoke blended Narrow advance 400 and Wide advance 800 into `H` advance 600 and wrote `Narrow / Wide @ 0.5`.
- Next: Troy picked UFO open/blend/save, the drawing window, and the MCP server. UFO landed in `82f747c`. The window and MCP landed in the Day 2 cloud slice.

### 2026-10-04 — UFO exchange

- Focus: finish the local UFO stub and keep every branch on `main`.
- Shipped UFO open, blend, and save on the existing commands, plus `move_point`. Components, images, and implied-on qcurves are refused. Tests use synthetic norad fonts.
- Validation: `powershell -ExecutionPolicy Bypass -File App/scripts/check.ps1` from the repo root.
- Next: the drawing window and the MCP server, in `documents/day-2-agent-prompt.md`. Generation stays frozen.

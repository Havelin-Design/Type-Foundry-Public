# Type Foundry

A local professional type kit. Design a font, blend two compatible faces into a new one, generate starting outlines from a prompt or an image, export real fonts, and drive all of that from plugins and from an agent.

## Folder map

- `App/` — Rust workspace. `crates/foundry-core` is the font model, blend, UFO exchange, and TrueType export. `crates/foundry-api` is the command session. `crates/foundry-cli` is the `foundry` binary. `crates/foundry-app` is the `typefoundry` drawing window. `crates/foundry-mcp` is the `foundry-mcp` stdio MCP server.
- `Agent/CONTEXT.md` — this file.
- `Design/` — the window as built, its chrome tokens, and editor direction.
- `documents/api.md` — the command contract.
- `documents/shift-reference.md` — what we take from Shift, and what we do not copy.
- `documents/Keys/` — credentials, gitignored. None yet.

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

The working file is JSON, `format` `typefoundry.font`, `version` 1. A glyph is contours of `on` and `off` points, an advance, and an optional unicode. `open`, `save`, `check`, and `blend` also read and write a `.ufo` directory through the same commands. UFO import keeps the default layer, sorts glyph names, and keeps the first Unicode value. Anchors, guidelines, kerning, groups, and lib data are ignored. Components, images, and implied-on qcurves are refused. `save` to a `.ttf` path writes an installable TrueType file: cubics become quadratics, open contours are closed with a straight edge, and Unicode outside the Basic Multilingual Plane is refused.

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

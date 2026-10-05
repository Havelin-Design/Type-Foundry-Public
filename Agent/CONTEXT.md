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

The working file is JSON, `format` `typefoundry.font`, `version` 1. A glyph is contours of `on` and `off` points, an advance, and an optional unicode. `open`, `save`, `check`, and `blend` also read and write a `.ufo` directory through the same commands. `open` also reads a Three.js typeface JSON file, a webfontjson file, `.ttf`, `.otf`, `.ttc`, `.otc`, and WOFF 1 (`.woff`, unpacked into an sfnt). WOFF2 and Embedded OpenType are refused. Saving over `.otf`, `.ttc`, `.otc`, `.woff`, `.woff2`, or `.eot` is refused. UFO import keeps the default layer, sorts glyph names, and keeps the first Unicode value. Anchors, guidelines, kerning, groups, and lib data are ignored. Components, images, and implied-on qcurves are refused. `save` to a `.ttf` path writes an installable TrueType file: cubics become quadratics, open contours are closed with a straight edge, and Unicode outside the Basic Multilingual Plane is refused.

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

- Rust owns the font. The `typefoundry` window is a client of `foundry-api`. A drag, a new rectangle, and a new oval are session commands.
- The JSON command stream is the plugin and agent API. `foundry-mcp` wraps it for chat clients. It does not get its own font mutations.
- Blend refuses incompatible outlines and reports why.
- House UI uses the Havelin v2 system surface for chrome. The glyph canvas stays neutral: BONE ground, black fill, FOCUS / AMBER / SIGNAL handles.
- A folder of filled SVG glyphs, each named with four hex digits (`0041.svg` is A), opens through `Font::load`. Tight crops are scaled so the flat x-height is 500 in a 1000-unit em and share one baseline.
- Toolbar icons are the Gravity UI set (MIT, Yandex), vendored as SVG. The window draws them locally and does not fetch them.
- The window, the executable, and the existing Start menu shortcut use Troy's mark at `App/crates/foundry-app/assets/type-foundry-icon.png`.
- Cargo target directory stays on `C:`.

## Session log

### 2026-10-05 — Several fonts and families (cloud)

- Focus: edit a Regular and an Italic side by side and ship them as one family. Branch `claude/font-families`.
- Model: every font has a `style` (family, style name, weight, italic, italic angle). Files without it load as the Regular of a family named after the font. TrueType, OpenType, and UFO import read it. TrueType and UFO export write it with proper style linking: name IDs 1, 2, 16, and 17, OS/2 weight and `fsSelection`, `head.macStyle`, `post.italicAngle`, and the `hhea` caret slope.
- Session: holds many open fonts, each with its own undo history and unsaved-changes flag. Existing commands act on the active font. New commands: `fonts`, `select_font`, `close_font`, `set_style`, `derive_style` (with an optional slant), `family_check`, `save_family` and `open_family` (a `typefoundry.family` file beside `Family-Style.json` members), and `export_family` (TTF, UFO, or JSON, refused on blocking issues). `glyph` and `index` take an optional `font`.
- MCP: 9 more tools for those commands. `font_save` with no path now remembers a path per font, so a derived style can never overwrite another style's file.
- Window: font tabs, New style dialog, Style section in the inspector, open, save, and export family, a family check report, a compare layer that draws another style behind the glyph, and a preview line per style.
- Validation: fmt, tests, and clippy `-D warnings` passed in the Linux container. Xvfb smoke on Roboto English: made an Italic at 12 degrees, compared the Italic and Regular R, previewed both, the check came back ready, and switching tabs kept the glyph. `foundry run` exported Regular, Italic, and Bold Italic TTFs. `fc-scan` read all three as one family, Roboto Draft, with the right weights and slants.
- Next: interpolating between styles, a weight axis with masters, and a variable-font export. Kerning stays later.

### 2026-10-04 — Pushed the drawing slice

- Focus: put the local slice on `main`.
- Commit `168f996` (`168f996bc9caa9e56b53b3c7238f8a98b0b419c8`) is on `main`. It adds rectangle and oval, the preview pane, Gravity UI toolbar icons, SVG folder import, and the window mark. Pushed `dbca862..168f996` to `https://github.com/thavelin/Type-Foundry.git` `HEAD:main` over HTTPS. Origin stays the SSH remote. The tracking line may still say `origin/main` is gone. That was left alone.
- Vostok outlines were not committed. The earlier session entries below still say "not pushed" because that was true when they were written.

### 2026-10-04 — Window icon

- Focus: use Troy's mark as the Type Foundry icon.
- The source is `E:\random\Type-Foundry-Icon.png`, a 256×256 image. A copy lives at `App/crates/foundry-app/assets/type-foundry-icon.png`. The window decodes it for the title-bar icon. The Windows build embeds that PNG in an icon resource, so the executable and the Start menu shortcut use it too. The existing shortcut still points at the C: release exe. Its icon location is that exe.
- Validation: `the_app_icon_is_the_square_mark` passed. The release exe contains the PNG, opened with the title `Type Foundry`, and was closed. The glyph view was not clicked.
- Not pushed.

### Correction - Window icon validation

- The full Windows check later passed: 73 tests (6 api, 12 app lib, 10 app bin, 1 cli, 38 core, 6 mcp), 1 ignored regen test, clippy clean. `the_app_icon_is_the_square_mark` is one of the 10 app-bin tests. The release exe that contains the PNG was linked before a one-line clippy change in `app_icon` (`as_chunks`). The icon pixels are the same, and that exe was not rebuilt after the line. Not pushed.

### 2026-10-04 — SVG folder to a font

- Focus: open Troy's first font, `T:\troy-freeform\Fonts\Vostok-Serif\SVG`, and line the letters up.
- `Font::load` reads a folder of filled SVGs. The file name is four hex digits (`0041.svg`, also `U+0041.svg` or `uni0041.svg`). A folder named `SVG` takes the parent name, so this one opens as Vostok Serif. Strokes that were not expanded to fills are refused. `usvg` 0.45 parses the paths with text features off.
- The 90 Vostok files are tight crops at one scale. Flat letters sit on y = 0. Round letters split their extra height as overshoot. `p`, `q`, and `g` hang from the round x-height; `y` hangs from the flat x-height; `j` shares the descender. The flat x-height becomes 500 units at 1000 UPM. Cap height, ascender, and descender are measured from the aligned ink. Each crop gets 40 units of sidebearing on both sides. A space is added at 250 because the folder has no `0020.svg`.
- File > Open SVG folder… picks the directory. `foundry info` accepts the same path.
- Validation: the Windows check passed. 72 tests (6 api, 21 app, 1 cli, 38 core, 6 mcp), 1 ignored regen test, clippy clean. The Vostok folder itself is one of those tests: 91 glyphs, x and H on the baseline, o overshoots, p descends, the period sits on the baseline, the comma hangs. The release window opened that folder, the process title was `Vostok Serif - Type Foundry`, and the process was closed. The glyphs were not clicked.
- Not pushed. The rectangle, oval, preview pane, and Gravity UI icons are in the same local tree. `main` is still `dbca862`.
- Deferred: kerning, a real space drawing, optical sidebearings per letter, and components. Generation stays frozen.
- Next: Troy looks at Vostok Serif in the window. Push waits until he asks.

### 2026-10-04 — Gravity UI toolbar icons

- Focus: replace the text-only toolbar and Tools menu with Gravity UI icons, kept beside the names.
- Nine SVGs from the Gravity UI set (MIT, Copyright (c) 2022 YANDEX LLC) live in `App/crates/foundry-app/icons`, with `LICENSE` beside them. `foundry-app` rasterizes them with `resvg` 0.45 (`default-features = false`). `currentColor` becomes white, then egui tints the icon with the chrome text color. The window does not fetch icons.
- Mapping: Overview `layout-cells`, Editor `pencil-to-square`, Select `location-arrow`, Pen `pencil`, Rectangle `square`, Oval `circle`, Undo `arrow-rotate-left`, Redo `arrow-rotate-right`, Effects `magic-wand`. Hover text still carries the shortcut sentence. The Tools menu uses the same four tool icons.
- Validation: the Windows check passed. 65 tests (6 api, 21 app, 1 cli, 31 core, 6 mcp), 1 ignored regen test, clippy clean. `icons::tests::every_toolbar_icon_rasterizes` passed. The release `typefoundry.exe` opened `Fonts/Roboto-English.json`, the process title was `Roboto English - Type Foundry`, and the process was closed. The icon buttons were not clicked. The Start menu shortcut was left alone.
- Not pushed. This sits on the same unpushed tree as the rectangle, oval, and preview pane. `main` is still `dbca862`. MCP is still the original 9 tools.
- Deferred: icon-only buttons, icons on the rest of the menus, and the other 790 Gravity UI icons. MCP edit commands, a collection face index, components, kerning, WOFF2, OTF export. Generation stays frozen.
- Next: Troy tries the toolbar. Push waits until he asks.

### 2026-10-04 — Rectangle, oval, and the preview pane

- Focus: glyph creation tools, and a preview that can hold headlines and paragraphs.
- Rectangle (R) and Oval (O) drag onto the current glyph. Each drag is one `add_contour`, so one undo. The box is normalized. A side shorter than 4 units is refused. On-curve rectangle corners are not smooth. The oval is four cubic quadrants (kappa 0.5522847498307936), 12 points, closed, first point at the right. An amber ghost follows the drag. Esc cancels it. The new points are selected.
- The bottom strip is now a resizable preview pane (`preview.rs`). The copy column is headline and paragraph blocks (add, remove, retype, switch role). The format sheet sets headlines at 48px and paragraphs at 15px, then a size waterfall of the first headline at 36, 24, 16, and 11. Lines wrap in `lay_text`. Missing characters stay a hollow box. Click a glyph in the sheet to select it. The copy persists under the eframe key `preview_copy`.
- While a text field is focused, Ctrl+Z stays with that field. Font undo still uses Ctrl+Z when nothing is being typed.
- `foundry-app/src/lib.rs` no longer claims the window writes only with `move_point`.
- Validation: the Windows check passed. 64 tests (6 api, 20 app, 1 cli, 31 core, 6 mcp), 1 ignored regen test, clippy clean. The release `typefoundry.exe` opened `Fonts/Roboto-English.json`, the title became `Roboto English — Type Foundry`, and the process was closed. Rectangle, oval, and the sheet were not clicked in the window. Their geometry and wrapping are covered by the new unit tests.
- Not pushed. The Start menu shortcut was left alone. MCP is still the original 9 tools.
- Deferred: MCP coverage of the edit commands, a collection face index, components, kerning, WOFF2, OTF export. Generation stays frozen.
- Next: Troy tries the new tools and the pane. More drawing tools wait on what he asks for.

### 2026-10-04 — Windows check of the editor

- Focus: PR #3 is on `main`. Confirm the editor on this PC.
- Local `main` fast-forwarded from `b43b9e5` to `521a3e0`. The branch adds edit commands and undo (`9c49359`), fixes typeface.js curve order and regenerates the two Roboto files (`d2c18f2`), and rebuilds the window (`362351f`).
- typeface.js `q` and `b` put the end point first and the control points after, which matches three.js `Font`. The Roboto subsets still load as 95 English glyphs and 255 Cyrillic glyphs.
- Validation: the Windows check passed. 57 tests (6 api, 13 app, 1 cli, 31 core, 6 mcp), 1 ignored regen test, clippy clean. The release `typefoundry.exe` opened `Fonts/Roboto-English.json` and the window title became `Roboto English — Type Foundry`. The process was then closed. The existing Start menu shortcut was left alone.
- Not re-done here: the Linux smoke of box select, nudge, slant, and the pen. Those remain self-reported. MCP is still the original 9 tools. `foundry-app/src/lib.rs` still says the window writes only with `move_point`; the window sends the new edit commands.
- Next: add the new edit commands to the MCP server. A collection face index is still open. Generation stays frozen.

### 2026-10-04 — Editor UI (cloud)

- Focus: turn the bare window into an editor. Branch `claude/editor-ui`.
- Engine: 20 new session commands (point, contour, glyph, and font edits, `transform` with a per-glyph `anchor`, `round_coordinates`, `index`, `undo`, `redo`, `checkpoint`, `history`). Edits check their input before changing anything. Undo keeps 200 steps, and drags coalesce into one step.
- Window: a menu bar, toolbar, overview grid of cached thumbnails, select and pen tools, box selection, Alt-click to split a segment, an inspector, an Effects dialog with a live preview, a text preview strip, a settings window, and keyboard shortcuts. Split into `app`, `canvas`, `grid`, `panels`, `effects`, and `settings` modules.
- Fixed on the way: the typeface.js importer read `q` and `b` control points before the end point, which scrambled every curve. The two `Fonts/Roboto-*.json` subsets were regenerated from the json-fonts source.
- Validation: fmt, `cargo test --workspace`, and clippy `-D warnings` passed in the Linux container. Smoke under Xvfb on Roboto English: overview, editor, box select plus nudge (one undo step), undo, a 12-degree slant on all 95 glyphs from Effects, a new glyph drawn with the pen and closed, a point added by Alt-click and dragged, smooth toggled, and the result saved.
- Next: the MCP server still lists its original 9 tools. Add the new edit commands there. Then component support, kerning, and a Bold effect (path offsetting).

### 2026-10-04 — WOFF 1, after the binary-import merge

- Focus: PR #2 is on main. Confirm it on this PC, then take the first next step in `documents/font-formats.md`.
- Confirmed `906c0d3` (merge of `f53daaf` onto `a1568d6`). The Windows check passed before any new code: 40 tests (4 api, 8 app, 1 cli, 21 core, 6 mcp), 1 ignored, clippy clean. Save As already lists TrueType in `save_as_dialog`. The native dialog was not clicked. Saving a `.ttf` through the session was already covered by `a_saved_ttf_opens_with_names_unicodes_and_points`.
- Shipped WOFF 1 in `sfnt::unpack_woff`. `flate2` inflates a table when its compressed length is shorter than the original, and a stored table is copied. The sfnt is rebuilt and `ttf-parser` reads it. WOFF2 stays refused. `.woff` stays read-only. A webfontjson file can embed WOFF 1. `documents/api.md`, `README.md`, and `Design/README.md` no longer say that raw `.woff2` opens.
- Validation: the Windows check passed again. 44 tests (core 25 passed, 1 ignored), clippy clean. The new tests wrap `write_ttf` output, compress at least one table, and compare outlines with the `.ttf` and with the hand-built CFF font. No third-party font.
- Deferred: picking a collection face, variable-font masters, WOFF2, OTF export, and pen and select tools. Generation stays frozen.
- Next: optional face index on `open`, `check`, and `blend`.

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

# Shift reference

Source: https://github.com/shift-editor/shift

Shift is a free font editor. Electron and TypeScript own the window. Rust owns the font model, the `.shift` SQLite document, and import/export. Public status on 2026-10-04:

- Drawing, components, UFO, Designspace, Glyphs import, TTF/OTF viewing, and variable TrueType export are in the app.
- Kerning and text proofing are marked planned.
- Scripting, an MCP server, and AI assist are on the future list in `ROADMAP.md`.

Use Shift as an architecture reference:

- One font model in Rust.
- A document you can reopen.
- UFO and binary fonts as interchange, not as the only working copy.
- A narrow bridge that the UI calls.

Do not vendor Shift, depend on its crates, or copy its source. Type Foundry's early surface is the command API, blending two documents, and later generation. Those are the gaps in Shift's own roadmap.

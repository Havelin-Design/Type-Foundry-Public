# Type Foundry

A local type-design kit. The same command API drives the font model, plugins, and AI. The first working piece is blending two compatible fonts into a new one.

This is not a fork of [Shift](https://github.com/shift-editor/shift). Shift is the reference for a modern editor: a Rust font core, with import and export around it. Scripting and an AI API are still on Shift's future list. Those are the parts this project builds first.

## Today

```text
foundry new --name "Wide" --upm 1000 --out wide.json
foundry check narrow.json wide.json
foundry blend narrow.json wide.json --t 0.5 --out mid.json
foundry blend Narrow.ufo Wide.ufo --t 0.5 --out Mid.ufo
foundry run --file commands.jsonl
```

A `save` command whose path ends in `.ttf` writes an installable TrueType file from the open font.

```json
{"op":"save","path":"Mid.ttf"}
```

`foundry run` reads one JSON command per line. That stream is the plugin and AI surface. See `documents/api.md`.

Blend is linear interpolation. Both fonts need the same glyph names, contour counts, point counts, and point types. That is the same rule variable-font masters use. Two unrelated typefaces will be refused until a later matching step exists. `open`, `save`, `check`, and `blend` accept a `.ufo` directory as well as the JSON working file. `save` also writes `.ttf`.

## Layout

- `App/` — Rust workspace. `foundry-core` holds the font, `foundry-api` runs commands, `foundry-cli` is the `foundry` binary.
- `Agent/CONTEXT.md` — project facts and the session log.
- `documents/api.md` — the command contract.
- `Design/` — editor direction, when the window exists.

Build output goes to `C:\Users\Troy Havelin\AppData\Local\typefoundry-target` because this share creates files without execute permission.

From `App/`:

```text
cargo run -p foundry-cli -- check a.json b.json
powershell -ExecutionPolicy Bypass -File scripts/check.ps1
```

Remote: `git@github.com:thavelin/Type-Foundry.git`

Hub: https://app.notion.com/p/3ef627d6cfdc81e4a936e4f714b7aff0

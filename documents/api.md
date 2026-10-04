# Command API

`foundry run` reads JSON commands, one per line, from a file or from stdin. Blank lines and lines that start with `#` are skipped. A leading byte-order mark is ignored. Each command prints one JSON response. In JSON, write Windows paths with forward slashes or escaped backslashes.

A plugin or an agent uses this stream. There is no second mutation API.

```text
foundry run --file commands.jsonl
```

## Response

```json
{"ok":true,"data":{"name":"Wide","upm":1000,"glyphs":[]}}
{"ok":false,"error":"glyph H is missing from Narrow","data":{"issues":[]}}
```

`ok` is false when the command did not do what was asked. Blend and check failures include `data.issues`.

## Commands

`create` starts an empty font in the session. `upm` defaults to 1000 and must be from 16 to 16384.

```json
{"op":"create","name":"Wide","upm":1000}
```

`open` and `save` read and write a project JSON file or a `.ufo` directory. `save` also writes an installable `.ttf` when the path ends in `.ttf`. `check` and `blend` compare or blend JSON and UFO. JSON stays the working document. A UFO import keeps the default layer, sorts glyph names, and keeps the first Unicode value. It ignores anchors, guidelines, kerning, groups, and lib data. It refuses a glyph that has components or an image, and it refuses a qcurve that is not exactly one off-curve point. One off-curve point is a quadratic. Two are a cubic. On the way out, the UFO family name is the font name. TrueType export turns cubics into quadratics, closes an open contour with a straight edge, and refuses a Unicode value outside the Basic Multilingual Plane.

`open`, `check`, and `blend` also read binary fonts: `.ttf`, `.otf` (CFF or CFF2 outlines), and the first face of a `.ttc` or `.otc` collection. The import keeps glyph order, names from `post` or the CFF charset, the lowest Unicode value per glyph, advances, units per em, and the vertical metrics. Composite glyphs are decomposed. TrueType implied on-curve points become real on-curve points. A variable font gives its default instance. Kerning, features, hinting, smooth flags, and other faces in a collection are not imported. A glyph without a stored name is called `uniXXXX`, or `glyphNNNNN` when it has no Unicode. `.woff` and `.woff2` are refused with a message that says so. Saving to `.otf`, `.ttc`, `.otc`, `.woff`, or `.woff2` is refused, so nothing writes JSON under a binary extension. TrueType export now stores glyph names (`post` format 2), so a saved `.ttf` opens with the same names. See `documents/font-formats.md`.

```json
{"op":"open","path":"wide.json"}
{"op":"save","path":"wide.ufo"}
{"op":"save","path":"wide.ttf"}
{"op":"open","path":"C:/fonts/Crimson Pro Regular.ttf"}
{"op":"open","path":"C:/fonts/Loma-Bold.otf"}
```

`info` describes the open font. `glyphs` lists names. `glyph` returns one glyph.

```json
{"op":"info"}
{"op":"glyphs"}
{"op":"glyph","name":"H"}
```

`put_glyph` inserts or replaces a glyph. This is the seam for prompt and image generation: a generator emits glyph JSON, and this command stores it.

```json
{"op":"put_glyph","glyph":{"name":"H","unicode":72,"advance":700,"contours":[{"closed":true,"points":[{"x":100,"y":0,"kind":"on","smooth":false},{"x":240,"y":0,"kind":"on","smooth":false},{"x":240,"y":700,"kind":"on","smooth":false},{"x":100,"y":700,"kind":"on","smooth":false}]}]}}
```

`set_advance` changes the advance of an existing glyph.

```json
{"op":"set_advance","name":"H","advance":680}
```

`move_point` sets one point to an absolute coordinate. `contour` and `point` are zero-based indexes.

```json
{"op":"move_point","name":"H","contour":0,"point":0,"x":110,"y":20}
```

`check` compares two files. `blend` writes a new file and opens it in the session. `t` defaults to 0.5. `t` is 0 at the first font and 1 at the second. Values outside that range extrapolate.

```json
{"op":"check","a":"narrow.json","b":"wide.json"}
{"op":"blend","a":"narrow.json","b":"wide.json","t":0.5,"out":"mid.json"}
```

## Project file

`format` is `typefoundry.font` and `version` is `1`. Points are `on` or `off`. A cubic segment is two `off` points between `on` points. A quadratic segment is one `off` point. Contours are closed or open.

## Blend rules

Blending refuses the pair when any of these differ: units per em, glyph name set, unicode, contour count, closed flag, point count, point kind, or smooth flag. The result keeps the first font's glyph order. Advances, coordinates, and vertical metrics are interpolated.

## CLI

These commands call the same operations:

```text
foundry new --name "Wide" --upm 1000 --out wide.json
foundry info wide.json
foundry check narrow.json wide.json
foundry blend narrow.json wide.json --t 0.5 --out mid.json
```

`check` and `blend` exit 1 when the fonts are not compatible.

## MCP server

`foundry-mcp` is a stdio MCP server over one command session. `foundry mcp` runs the same server. A chat client can create, open, inspect, move a point, check, blend, and save. There are no generation tools.

Every tool runs on local files only. No tool uploads a font, an outline, or anything else, and the server makes no network calls.

Stdout carries only protocol messages, one JSON-RPC object per line. Logs go to stderr. The server answers `initialize` (it echoes the client's `protocolVersion`), `ping`, `tools/list`, and `tools/call`. A message without an `id` is a notification and gets no reply.

| Tool | Arguments | Command |
| --- | --- | --- |
| `font_create` | `name`, `upm?` | `create` |
| `font_open` | `path` | `open` |
| `font_save` | `path?` | `save` |
| `font_info` | none | `info` |
| `font_glyphs` | none | `glyphs` |
| `glyph_get` | `name` | `glyph` |
| `point_move` | `name`, `contour`, `point`, `x`, `y` | `move_point` |
| `font_check` | `a`, `b` | `check` |
| `font_blend` | `a`, `b`, `t?`, `out` | `blend` |

A tool result is `{"content":[{"type":"text","text":"..."}],"isError":false}`. The text is the command response JSON. `isError` is true when the command response has `ok: false`, or when the arguments do not fit the tool.

`font_save` takes the same paths as `save`, including `.ttf`. Without a `path` it saves to the last path this server process opened, saved, or blended to. `font_create` clears that path, so a new font needs one explicit `path` the first time. The path lives in the MCP wrapper, not in the session.

Claude Desktop (`claude_desktop_config.json`) or Cursor (`.cursor/mcp.json`):

```json
{
  "mcpServers": {
    "typefoundry": {
      "command": "C:/Users/Troy Havelin/AppData/Local/typefoundry-target/release/foundry-mcp.exe",
      "args": []
    }
  }
}
```

Claude Code:

```text
claude mcp add typefoundry -- "C:/Users/Troy Havelin/AppData/Local/typefoundry-target/release/foundry-mcp.exe"
```

Build it first, from `App/`: `cargo build --release -p foundry-mcp`. Pass tool paths with forward slashes, for example `{"path":"C:/fonts/Wide.ufo"}`.

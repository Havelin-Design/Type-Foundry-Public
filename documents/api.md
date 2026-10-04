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

`open` and `save` read and write the project JSON.

```json
{"op":"open","path":"wide.json"}
{"op":"save","path":"wide.json"}
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

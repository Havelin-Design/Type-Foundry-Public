# Font formats

Every format is read into, and written from, the same `typefoundry.font` model. JSON stays the working document. Nothing here is a second editor model. All of it runs locally, and nothing is uploaded.

## What works

| Format | Open | Save | Notes |
| --- | --- | --- | --- |
| `.json` (`typefoundry.font`) | yes | yes | The working document. Lossless. |
| `.ufo` (UFO 2 and 3) | yes | yes (UFO 3) | Default layer only. Components and images are refused. |
| `.ttf` (TrueType) | yes | yes | Import decomposes composites and keeps names. Export turns cubics into quadratics. |
| `.otf` (OpenType CFF or CFF2) | yes | no | Cubic outlines come in as two off-curve points. |
| `.ttc`, `.otc` (collections) | face 0 | no | Other faces are not read yet. |
| `.woff`, `.woff2` | refused | no | The error says to convert to `.ttf` or `.otf` first. |

`ttf-parser` 0.25 reads binary fonts. It was already in the workspace as the export test reader. It draws outlines through a pen, so the importer collects `move`, `line`, `quad`, `curve`, and `close` into closed contours. Reading the `post` table in one pass keeps a 45,000-glyph CJK collection to about a third of a second. The parser's own name lookup is quadratic.

## What a binary import loses

Binary fonts are built for shipping, not editing. These things are not in the file, or are not brought in:

- **Smooth flags.** TrueType and CFF do not store them, so every point comes in as not smooth. Guessing from collinear handles could differ between two masters and break blending, so the importer does not guess.
- **Implied on-curve points.** TrueType can run two off-curve points in a row with an implied on-curve point between them. That point becomes a real on-curve point, which is the model's rule.
- **Components.** Composite glyphs (accented letters) are decomposed into outlines. UFO import refuses components instead, because a UFO is an editing source and decomposing it would hide a choice.
- **Variation data.** A variable font gives its default instance. Masters and axes are not imported.
- **Kerning, features (GSUB and GPOS), hinting, anchors, and the full name table.** None of these are in the model yet.

Two binary fonts blend only when their outlines are compatible, as with any other source. Static fonts from the same family are often not compatible, because their builds differ in glyph sets and point structure. `foundry check` says why.

## Possible next steps, in order of value

1. **WOFF 1.** A zlib wrapper around the same tables. It needs `flate2` and about 60 lines to rebuild the sfnt, then the existing importer reads it.
2. **Pick a face in a collection.** Add an optional `face` index to `open`, `check`, and `blend`, and list the faces in the error when a collection has more than one.
3. **Variable font instances and masters.** `ttf-parser` can set axis coordinates. Reading named instances as separate sources would give blend real masters from one file, which is where it matters most.
4. **WOFF 2.** Brotli plus the `glyf` and `loca` transforms. It needs a Brotli crate and a table rebuild, so it costs more than WOFF 1.
5. **OTF export.** CFF writing keeps cubics exact, where the TrueType path approximates them.
6. **Not planned:** Type 1 (`.pfa`, `.pfb`), `.dfont`, and bitmap formats. They are legacy, and a converter handles them better than this kit would.

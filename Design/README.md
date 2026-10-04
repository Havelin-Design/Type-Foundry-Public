# Design

The editor window is `typefoundry` (crate `App/crates/foundry-app`). It is a client of `foundry-api`. It reads the font with the `open`, `info`, and `glyph` commands and changes it only with `move_point` and `save`. It never writes font fields itself.

## The window

- **Toolbar** — Open… (a Type Foundry `.json`, a Three.js typeface `.json`, a webfontjson file, or a `.ttf` / `.otf` / `.woff` / `.woff2`), Open UFO… (a `.ufo` folder), Save, Save As…, then the font name and the path it came from. Save writes back to that path. Save As picks `.json` or `.ufo` by the extension you type. A web font that is not `.ttf` has to be saved as `.json`, `.ufo`, or `.ttf`. Dialogs start in the last folder used, which the window remembers between launches. The session does not hold the path.
- **Glyph list** — the left panel lists glyph names in font order. Click one to draw it.
- **Canvas** — the selected glyph, fitted to the canvas with the advance box and vertical metrics in view. Scroll or pinch zooms around the pointer. Right- or middle-drag pans. Double-click refits.
- **Points** — drag a handle to move it. Each drag step sends `move_point` with absolute font coordinates rounded to whole units, then redraws from the session. When an on-curve and an off-curve handle overlap, the on-curve handle is picked.
- **Status bar** — the last result in SIGNAL, or the error in ALERT, plus the picked point's contour, index, and coordinates.

A path given on the command line opens on launch: `typefoundry C:/fonts/Wide.ufo`.

On first launch on Windows the window adds `Type Foundry` to the Start menu, pointing at `C:\Users\Troy Havelin\AppData\Local\typefoundry-target\release\typefoundry.exe`. It skips that when the shortcut already exists.

## Chrome

Chrome follows the Havelin v2 system surface used by Frame Extractor. Only these colors are used:

| Token | Hex | Use |
| --- | --- | --- |
| PAGE | `#030303` | Canvas surround, text fields |
| PANEL | `#090907` | Toolbar, glyph list, status bar |
| RAISED | `#11110D` | Buttons |
| HAIRLINE | `#302A1E` | Panel borders, button edges |
| HAIRLINE_STRONG | `#5B4A2D` | Hovered button edge |
| INK | `#DED9CE` | Text |
| MUTED | `#9D988C` | Secondary text, metric lines |
| BONE | `#F3EEE4` | Canvas ground |
| FOCUS | `#D8FF00` | On-curve handles, selection, pressed edge |
| AMBER | `#E8B65A` | Off-curve handles and their tethers |
| SIGNAL | `#8AE6A3` | Selected handle, success status |
| ALERT | `#F07461` | Error status |
| INVERSE | `#030303` | Text on FOCUS |

Fonts are egui's defaults. Inter and IBM Plex are not vendored.

## Canvas

The glyph canvas stays neutral: BONE ground, black fill (nonzero winding, so counters stay open), MUTED metric lines. On-curve handles are FOCUS circles, off-curve handles are AMBER squares, and the selected handle is SIGNAL. Each handle has a thin black rim so FOCUS reads on BONE. Open contours draw as a black stroke.

## Next

Interaction ideas worth borrowing from Shift, without copying its UI: select, pen, and hand tools, a glyph grid, and masters as sources you can preview between.

The product to design toward is a bench for making a font, blending two compatible faces, generating a starting face from a prompt or an image, and handing the same commands to a plugin or an agent.

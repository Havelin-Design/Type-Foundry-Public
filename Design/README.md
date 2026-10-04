# Design

The editor window is `typefoundry` (crate `App/crates/foundry-app`). It is a client of `foundry-api`. It reads and changes the font through session commands; it never writes font fields itself.

## The window

The window opens on the **overview**: every glyph as a thumbnail at one shared scale. Double-click a glyph, press Enter, or press Tab to open it in the **editor**. The menus hold everything, and Help > Keyboard shortcuts lists the keys.

- **Menu bar.** File (New font, Open, Open UFO folder, Save, Save As, Quit), Edit (Undo and Redo with step counts, select all, deselect, delete points, smooth or corner, on-curve or off-curve, reverse contour direction), View (overview or editor, zoom, fit, panel and canvas toggles, Settings), Glyph (new, delete, previous, next), Tools (Select, Pen), Effects (Transform, the six effects, round coordinates), and Help.
- **Toolbar.** Overview and Editor, Select and Pen, Undo and Redo, Effects, then the font name, a dot when there are unsaved changes, and the current glyph. File commands open Type Foundry or Three.js JSON, webfontjson, `.ttf`, `.otf`, `.ttc`, `.otc`, and `.woff` files, or a `.ufo` folder; Save As writes Type Foundry JSON, UFO, or TrueType. The last folder persists between launches.
- **Overview.** A filter by name or character, a cell-size slider, and New glyph. Thumbnails are drawn once and cached until that glyph changes.
- **Glyph list.** The left panel, with its own filter. Click to select, double-click to edit.
- **Editor canvas.** Metric lines with labels, the advance box, black fill (nonzero, so counters stay open), and handles. Scroll or pinch zooms around the pointer, right- or middle-drag pans, and double-click on empty space refits. A panel opening or resizing does not move the glyph under the pointer.
- **Select tool (V).** Click a point to select it, Shift-click to add or remove, drag empty space to box-select. Drag the selection to move it. Arrows nudge 1 unit, Shift-arrows 10. Delete removes points. Alt-click an outline to add a point there without changing the shape. Double-click an on-curve point to toggle smooth.
- **Pen tool (P).** Click to start a contour and add on-curve points, Shift-click for off-curve points. Click the first point to close the contour. Esc stops.
- **Inspector.** Font name, units per em, and the four metrics. The glyph's name, Unicode, and advance, with sidebearings, ink box, and counts shown, plus Reverse, Round, and Delete. For one selected point: X, Y, on-curve or off-curve, and smooth. For several: Smooth, Corner, Delete, and Transform.
- **Effects (Ctrl+E).** Slant, Scale, Rotate, Move, Flip horizontal, and Flip vertical, applied to the selected points, this glyph, or all glyphs. An AMBER outline previews the result on the canvas before Apply. Apply is one `transform` command, so one undo step even for the whole font.
- **Preview strip.** Type any text to see it set in the font. Characters the font lacks show as a hollow box. Click a glyph in the strip to select it.
- **Settings (Ctrl+,).** Fill, stroke, metrics, point numbers, pointer coordinates, snapping moves to whole units, handle size, overview cell size, preview size, and which panels show. Settings and the last folder persist between launches.
- **Status bar.** The last result in SIGNAL, or the error in ALERT, plus the tool, the selection count, and the zoom.

Every change goes through a session command (`move_points`, `split_segment`, `add_contour`, `transform`, `set_metrics`, and the rest in `documents/api.md`), so undo covers all of it. The same commands are open to plugins and the CLI. The window never writes font fields itself. On-curve corner points draw as diamonds and smooth points as circles. A ring marks each contour's first point.

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

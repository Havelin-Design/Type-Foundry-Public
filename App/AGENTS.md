# Type Foundry

Product code is this folder. Facts and the session log live in `../Agent/CONTEXT.md`. The command contract is `../documents/api.md`.

Mutations go through `foundry-api`. Do not add a second way to change a font.

Do not copy Shift source or another foundry's outlines into this repo. Local font authoring only. Do not upload fonts, outlines, reference images, or prompts.

Cargo writes build output to `C:\Users\Troy Havelin\AppData\Local\typefoundry-target`. Run `powershell -ExecutionPolicy Bypass -File scripts/check.ps1` before calling the engine done. This machine's execution policy requires the bypass.

---
level: L2
path: troy-freeform/TypeFoundry
summary: Local type foundry — design, blend, generate, and export fonts through one Rust command API.
status: active
owner: Troy
updated: 2026-10-04
---

# TypeFoundry — Context

<!-- HUMAN/AI NARRATIVE LAYER — the indexer never overwrites this file.
     Fill in the sections below. Keep the frontmatter `summary:` current:
     parent _MAP.md files quote it for progressive disclosure. -->

## Purpose

A local professional type kit. The Rust engine stores fonts, blends compatible faces, and exposes one command stream for the future editor, plugins, and agents.

## What lives here

Product code is in `App/`. Project facts are in `Agent/CONTEXT.md`. Research and the API contract are in `documents/`. Shift is a reference only and is not vendored here.

## Conventions

Working fonts are `typefoundry.font` JSON. Cargo build output stays on `C:` because this share strips execute permission. Do not copy another foundry's outlines into the repo.

## Key links & owners

- Repo: https://github.com/thavelin/Type-Foundry
- Hub: https://app.notion.com/p/3ef627d6cfdc81e4a936e4f714b7aff0
- Owner: Troy

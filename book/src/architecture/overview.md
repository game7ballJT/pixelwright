# Architecture overview

Pixelwright separates the document engine from presentation and transport. The desktop UI, CLI, JSON control channel, and MCP server reach editing behavior through the command registry rather than maintaining separate implementations.

```text
desktop UI       CLI       JSON control       MCP
    |             |              |              |
    +-------------+--------------+--------------+
                          |
                   command registry
                          |
                    engine Session
                          |
       document model + operations + compositors
                          |
             PSD / raster / .pxwr I/O
```

The enforced dependency direction runs from format-independent foundation crates through the document and operation layers to composition, I/O, the engine, and finally UI/automation. `cargo xtask layers` checks this rule.

The complete design narrative and dependency diagram remain in [`docs/architecture.md`](https://github.com/storytold/photocraft/blob/main/docs/architecture.md). That document contains historical design material as well as implemented architecture; confirm behavior in current source when security or compatibility depends on it.

## Core properties

- `pixelwright-doc` holds the pure-data document model.
- `pixelwright-engine::Session` owns open documents, active state, history, and command dispatch.
- `pixelwright-compose` is the CPU reference compositor; `pixelwright-gpu` accelerates supported interactive paths.
- `pixelwright-io` maps between documents and PSD or flat formats.
- `pixelwright-format` reads and writes the native `.pxwr` bundle.
- `pixelwright-ui-egui` is a thin shell over engine commands.
- `pixelwright-automation` exposes the headless engine and the live-GUI bridge.

See [Crates and dependency layers](crates.md) for the complete workspace map.

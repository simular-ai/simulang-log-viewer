# Examples

Runnable snippets demonstrating `@simular-ai/simulang-log-viewer`. All files are plain ESM JavaScript; no TypeScript toolchain or extra dependencies needed at runtime. They are typechecked at build time via `examples/tsconfig.json`.

## How to run

After installing the package in your project:

```bash
node node_modules/@simular-ai/simulang-log-viewer/examples/with-simulang-js.mjs
```

Or, if you want to tweak them, copy the file into your own project and run it from there:

```bash
cp node_modules/@simular-ai/simulang-log-viewer/examples/with-simulang-js.mjs ./
node with-simulang-js.mjs
```

## Examples

| File                   | What it does                                                                                                                                                                                                         |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `with-simulang-js.mjs` | Auto-install flow: importing the package spawns the singleton window, registers simulang-js's pause hook, and wires log forwarding. Demonstrates pause-aware simulang-js calls and `logWindow.log` for custom lines. |
| `basic.mjs`            | Full LogWindow API tour via the `/binding` escape hatch (`spawn`, `log`, `waitIfPaused`, `waitIfPausedSync`, `clear`, `close`). Pick this when you want manual control.                                              |

## Heads up

- The window is always-on-top by default. It floats above other apps until you close it.
- On macOS the window is excluded from screen capture (`NSWindowSharingNone`), so screenshots and screen recordings won't include it.
- Click-through by default. The window never intercepts mouse input until the user presses the global pause hotkey shown inside the window (`Ctrl+Shift+Option+L` on macOS, `Ctrl+Shift+Alt+L` elsewhere — same key combo, different keycap name); that pauses execution and lets the user drag the window via its native title bar. Pressing the same hotkey again resumes and re-enables click-through.
- Closing the parent process always shuts the viewer down via stdin EOF; calling `logWindow.close()` (or `win.close()`) explicitly is optional.

// Side-effect import that auto-installs the log viewer:
//
//   1. Spawns a singleton `LogWindow` subprocess.
//   2. Registers `LogWindow.waitIfPausedSync` as a pause hook on
//      `@simular-ai/simulang-js`, so every simulang-js method and
//      function transparently blocks while the user has paused
//      execution via the hotkey shown inside the window.
//   3. Forwards Rust-side logs from simulang-js into the viewer via
//      `initLogger`, formatted as `[level] message`.
//
// The package's `main` points here, so importing
// `@simular-ai/simulang-log-viewer` from anywhere — including just
// `import '@simular-ai/simulang-log-viewer'` for the side effect —
// wires everything up exactly once. Module evaluation runs once per
// Node process, so the spawn + hook registration is idempotent.
//
// `index.js` and `index.d.ts` remain the napi-rs codegen output
// verbatim (auto-doc and Claude-skill tooling that consumes them
// keeps working unchanged). Bypass the auto-install entirely by
// requiring `'@simular-ai/simulang-log-viewer/binding'` (mapped to
// `./index.js` in the package exports) — that gives you the raw
// `LogWindow` class with no side effects.

const binding = require('./index.js')
const sim = require('@simular-ai/simulang-js')

const logWindow = new binding.LogWindow()
logWindow.spawn()

sim.setPauseHook(() => {
  logWindow.waitIfPausedSync()
})
// Default the log filter to `simulang_rs=info,warn` so the viewer shows
// the user-facing actions emitted by simulang-rs (mouse, keyboard,
// clipboard, app launches) without pulling in info-level chatter from
// every other crate in the process. Set `RUST_LOG` to override.
//
// `enigo=warn` is appended unconditionally so the input-simulation
// backend stays quiet even when the user raises the global level (e.g.
// `RUST_LOG=debug`) to debug simulang internals.
const logSpec = `${process.env.RUST_LOG || 'simulang_rs=info,warn'},enigo=warn`
// `logWindow.log` mirrors every line to stderr internally, so a single
// call here gets the record into both the floating viewer and the
// terminal — and keeps flowing to the terminal if the user closes the
// viewer mid-run.
sim.initLogger((rec) => {
  logWindow.log(`[${rec.level}] ${rec.message}`)
}, logSpec)

module.exports = {
  ...binding,
  logWindow,
}

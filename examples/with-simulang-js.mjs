// Run: node examples/with-simulang-js.mjs
//
// The point of this example: importing `@simular-ai/simulang-log-viewer`
// is enough. A single side-effect `import` is all the log window need to start — the
// auto-install entry point handles three things on first evaluation:
//
//   1. Spawns the singleton log window.
//   2. Registers a synchronous pause hook on `@simular-ai/simulang-js`,
//      so every simulang-js method/free-function call blocks while the
//      user has paused execution by pressing the hotkey shown inside
//      the window (Ctrl+Shift+Opt+L on macOS, Ctrl+Shift+Alt+L
//      elsewhere).
//   3. Routes Rust-side `log::*!` records from simulang-rs into the
//      window via `initLogger`, formatted `[level] message`,
//      and mirrors every line to the host's stderr so the terminal
//      sees the full transcript even after the user closes the
//      floating window.
//
// The clipboard loop below contains zero logging code, yet every
// `setString` / `getString` call shows up live in both the window and
// the terminal because simulang-rs emits records through the auto-wired
// logger. The only thing this script does explicitly is drive
// simulang-js — the viewer just observes.
//
// Need to push custom lines too? Import the `logWindow` singleton from
// the same package and call `logWindow.log('...')`. See `basic.mjs` for
// the manual-control variant that constructs its own `LogWindow`.
//
// Requires `@simular-ai/simulang-js` installed alongside in the
// consuming project — it's the package's only peer dependency.

import '@simular-ai/simulang-log-viewer'
import { Clipboard } from '@simular-ai/simulang-js'

const cb = new Clipboard()

// Press the pause hotkey at any point and the next simulang-js call
// (and every one after) will block until you press it again — the
// pause hook installed by the side-effect import wires this up
// transparently.
for (let i = 0; i < 5; i++) {
  cb.setString(`tick ${i}`)
  await sleep(1000)
}
cb.getString()

await sleep(2000)

/** @param {number} ms */
function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

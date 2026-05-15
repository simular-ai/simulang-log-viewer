// Run: node examples/basic.mjs
//
// The "I want full control" path: construct a LogWindow yourself
// instead of using the auto-installed singleton. Importing from the
// `/binding` subpath skips the side-effect that the package's main
// entry runs (spawn window + register pause hook + wire initLogger).
//
// Pick this entrypoint when you want:
//   - to register your own `initLogger` callback, or
//   - just to keep imports explicit.
//
// API tour:
// - `spawn`         launches the viewer subprocess
// - `log`           appends a line
// - `waitIfPaused`  async — yields the event loop while paused
// - `waitIfPausedSync` synchronous — freezes the calling thread until
//                   the user releases the pause hotkey (this is the
//                   primitive that simulang-js's pause hook calls)
// - `isPaused`      cheap getter for state polling
// - `clear`         drops all displayed messages
// - `close`         shuts the viewer down (optional — also exits on
//                   parent process death)

import { LogWindow } from '@simular-ai/simulang-log-viewer/binding'

const win = new LogWindow()
win.spawn()

for (let i = 0; i < 10; i++) {
  win.log(`step ${i}`)
  await sleep(500)

  // Respect a user-initiated pause. The user enters/leaves the paused
  // state by pressing the pause hotkey shown inside the window. Returns
  // immediately if not paused.
  await win.waitIfPaused()
}

win.log('clearing buffer in 2s')
await sleep(2000)
win.clear()
win.log('done')

await sleep(500)
win.close()

/** @param {number} ms */
function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

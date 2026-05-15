# `@simular-ai/simulang-log-viewer`

A floating, always-on-top log window for automation scripts. Importing it once opens the window and shows every log line from your script and from `@simular-ai/simulang-js`.

## Usage

```js
import { logWindow } from '@simular-ai/simulang-log-viewer'
import { Clipboard } from '@simular-ai/simulang-js'

logWindow.log('--- starting work ---')

const cb = new Clipboard()
cb.setString('hi')
logWindow.log(`clipboard now: ${cb.getString()}`)
```

That's it. The import opens the window, and:

- `logWindow.log(msg)` writes your own lines into it.
- Rust-side logs from `simulang-js` show up automatically.
- Pressing the pause hotkey (`Ctrl+Shift+Option+L` on macOS, `Ctrl+Shift+Alt+L` elsewhere) pauses every `simulang-js` call until you press it again. The window also becomes draggable while paused.

The window is click-through by default, always-on-top, and excluded from screenshots on macOS so it never interferes with what you're automating.

## API

| Member               | Description                                                            |
| -------------------- | ---------------------------------------------------------------------- |
| `logWindow.log(msg)` | Append a line.                                                         |
| `logWindow.clear()`  | Drop all displayed messages.                                           |
| `logWindow.isPaused` | `true` while execution is paused via the hotkey; `false` otherwise.    |
| `logWindow.close()`  | Shut down the viewer. Optional — happens automatically on parent exit. |

## Peer dependency

`@simular-ai/simulang-js >= 6.0.0`. Install it alongside this package.

## Examples

Runnable examples live under [`examples/`](./examples/). Start with [`examples/with-simulang-js.mjs`](./examples/with-simulang-js.mjs).

## Development

```sh
npm install
npm run build:debug
node examples/with-simulang-js.mjs
```

## Releasing

See [`PUBLISH.md`](./PUBLISH.md). Releases are cut from CI on tag push.

## License

[MIT](./LICENSE) © 2026 Simular AI

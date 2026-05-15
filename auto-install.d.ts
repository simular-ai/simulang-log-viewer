/**
 * Hand-authored entry that re-exports the napi-generated `LogWindow`
 * class and adds the auto-installed singleton instance.
 *
 * Importing this module spawns a `LogWindow`, registers a pause hook
 * on `@simular-ai/simulang-js`, and routes Rust-side logs into the
 * viewer — see the package README for details. `index.d.ts` is the
 * napi-rs codegen output (and the source of truth for the public
 * API); this file is the wired-up entry that the package's `types`
 * field points to.
 */

export * from './index'

import { LogWindow } from './index'

/**
 * The auto-installed `LogWindow` instance. Already spawned and wired
 * up to `@simular-ai/simulang-js`'s pause hook + logger relay. Use it
 * directly to push your own log lines, clear the viewer, or close it.
 *
 * ```ts
 * import { logWindow } from '@simular-ai/simulang-log-viewer'
 * logWindow.log('starting work')
 * ```
 */
export declare const logWindow: LogWindow

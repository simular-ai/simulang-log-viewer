// Local dev helper: after `cargo build [--release] --bin simulang-log-viewer`,
// copy the resulting binary out of cargo's target directory into the
// package root, where it sits next to the locally-built `.node` file. The
// Rust path-discovery code then finds it via `dladdr` for `npm run *` and
// `node examples/...` invocations.
//
// In CI the per-platform binary is uploaded as a build artifact and then
// distributed by `scripts/distribute-viewer.mjs` (during `npm run
// artifacts`). This script is only used during local development and
// debugging.
//
// Honours `$CARGO_TARGET_DIR` (Cursor's agent sandbox redirects target/
// builds to a per-session cache, and CI may set it per matrix entry) and
// falls back to `./target/` otherwise.
//
// Usage: `node scripts/copy-viewer-local.mjs <debug|release>`

import { copyFileSync, chmodSync, existsSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const profile = process.argv[2]
if (profile !== 'debug' && profile !== 'release') {
  console.error('usage: node scripts/copy-viewer-local.mjs <debug|release>')
  process.exit(1)
}

const here = resolve(fileURLToPath(import.meta.url), '..', '..')
const targetDir = process.env.CARGO_TARGET_DIR ?? join(here, 'target')
const exe = process.platform === 'win32' ? 'simulang-log-viewer.exe' : 'simulang-log-viewer'

const src = join(targetDir, profile, exe)
const dst = join(here, exe)

if (!existsSync(src)) {
  console.error(`viewer binary not found at ${src} — did the cargo build succeed?`)
  process.exit(1)
}

copyFileSync(src, dst)
if (process.platform !== 'win32') {
  chmodSync(dst, 0o755)
}
console.log(`copied ${src} -> ${dst}`)

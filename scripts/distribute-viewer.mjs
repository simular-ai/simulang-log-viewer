// Publish-time helper: after `napi artifacts` distributes the per-platform
// `.node` files into `npm/<target>/`, this script does the same for the
// per-platform `simulang-log-viewer[.exe]` binaries.
//
// CI uploads each platform's viewer binary as part of the same build
// artifact bundle as its `.node` (so `actions/download-artifact` extracts
// both into `./artifacts/<artifact-name>/`). `napi artifacts` already
// knows which target a `.node` file belongs to and routes it; we mirror
// that mapping for the executable.
//
// Layout after `npm run artifacts`:
//
//   npm/
//     darwin-arm64/
//       simulang-log-viewer.darwin-arm64.node       <- placed by napi artifacts
//       simulang-log-viewer                         <- placed by THIS script
//     win32-x64-msvc/
//       simulang-log-viewer.win32-x64-msvc.node
//       simulang-log-viewer.exe
//     ...

import { readFileSync, writeFileSync, readdirSync, copyFileSync, chmodSync, existsSync, statSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = resolve(fileURLToPath(import.meta.url), '..', '..')
const npmDir = join(here, 'npm')
const artifactsDir = join(here, 'artifacts')

if (!existsSync(npmDir)) {
  console.error(`npm/ directory does not exist at ${npmDir} — run \`napi create-npm-dirs\` first`)
  process.exit(1)
}
if (!existsSync(artifactsDir)) {
  console.error(`artifacts/ directory does not exist at ${artifactsDir} — nothing to distribute`)
  process.exit(0)
}

const targets = readdirSync(npmDir).filter((d) => statSync(join(npmDir, d)).isDirectory())
let copied = 0
let missing = []

for (const target of targets) {
  const targetDir = join(npmDir, target)
  const pkgJson = JSON.parse(readFileSync(join(targetDir, 'package.json'), 'utf8'))
  // napi-rs writes the OS into pkgJson.os and the cpu/libc into pkgJson.cpu/libc.
  // The executable's filename is the same `binaryName` as the .node, with the
  // `.exe` suffix on Windows.
  const isWindows = pkgJson.os?.includes('win32')
  const exe = isWindows ? 'simulang-log-viewer.exe' : 'simulang-log-viewer'

  // Find the binary in the artifact bundle. CI uploads the artifact bundle
  // as `bindings-<rust-target-triple>` (matching simulang-js's convention),
  // so we look in `artifacts/bindings-*/`. The triple isn't directly in
  // the per-platform package's name (e.g. `darwin-arm64` vs
  // `aarch64-apple-darwin`), so we just check every artifact dir for a
  // matching exe.
  const candidates = readdirSync(artifactsDir)
    .map((d) => join(artifactsDir, d, exe))
    .filter((p) => existsSync(p))

  // Pick the candidate whose sibling .node file matches this target's
  // .node filename. That binds the executable to the correct
  // architecture even when multiple artifact bundles contain a binary of
  // the same name (e.g. the macOS x64 and arm64 builds both produce
  // `simulang-log-viewer`).
  const expectedNode = `simulang-log-viewer.${target}.node`
  const matching = candidates.find((p) => existsSync(join(p, '..', expectedNode)))

  if (!matching) {
    missing.push(target)
    continue
  }

  const dst = join(targetDir, exe)
  copyFileSync(matching, dst)
  if (!isWindows) {
    chmodSync(dst, 0o755)
  }

  // Add the executable to the per-platform package's `files` array so it
  // gets included in the published tarball. `napi create-npm-dirs` only
  // lists the .node file by default; this is the moral equivalent of what
  // it would generate if it knew about peer binaries.
  if (!Array.isArray(pkgJson.files)) pkgJson.files = []
  if (!pkgJson.files.includes(exe)) {
    pkgJson.files.push(exe)
    writeFileSync(join(targetDir, 'package.json'), JSON.stringify(pkgJson, null, 2) + '\n')
  }

  console.log(`copied ${matching} -> ${dst}`)
  copied++
}

if (copied === 0) {
  console.error('no viewer binaries copied — check artifact uploads in CI')
  process.exit(1)
}
if (missing.length > 0) {
  console.warn(`missing viewer binary for ${missing.length} target(s): ${missing.join(', ')}`)
}

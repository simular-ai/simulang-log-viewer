## Publishing

This package is published as a **public** package to the npm registry under the `@simular-ai` org.

- Package: `@simular-ai/simulang-log-viewer`
- Visibility: public

See [`README.md`](./README.md) for **consumer** install instructions. This doc only covers cutting a release.

### Prerequisites

- Releases are done exclusively from CI — never locally.
- Pushing a new tag starts the release process.
- Tag names must match:
  - Release: `x.y.z` (published with the `latest` tag)
  - Pre-release: `x.y.z-*` (published with the `next` tag)
- CI must pass the build job — per-platform artifacts are uploaded there and consumed by the publish job.

### How to publish a new version

1. Bump the version:

```
npm version [<newversion> | major | minor | patch | premajor | preminor | prepatch | prerelease [--preid=<prerelease-id>] | from-git]
```

2. Push commits and tags:

```
git push
git push --tags
```

`npm version` creates the commit and tag for you. CI publishes when a tag matching `x.y.z` or `x.y.z-*` is pushed.

### What gets published

For each release this package ships **two artifacts per target triple**:

1. The napi `.node` file — the bindings that Node loads.
2. The `simulang-log-viewer` executable — the eframe app that runs as the actual window.

Both are produced by the same Cargo crate (cdylib + bin), built side-by-side per-platform in CI, and bundled into the same per-platform npm subpackage:

```
@simular-ai/simulang-log-viewer-darwin-arm64/
  simulang-log-viewer.darwin-arm64.node
  simulang-log-viewer
```

The `LogWindow` Rust class discovers its sibling binary at runtime via `dladdr` (Unix) / `GetModuleHandleExW` (Windows), so consumers don't have to know about the binary's path.

### Notes

- The publish workflow authenticates to npmjs.org using the `NPM_TOKEN` repository secret (an automation token scoped to the `@simular-ai` org).
- Publishing is **skipped** if the tag name does not match the version rules above.

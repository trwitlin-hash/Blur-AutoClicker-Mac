# Building From Source

This fork is macOS-only and targets Apple Silicon (`aarch64-apple-darwin`).
For Windows, use [upstream](https://github.com/Blur009/Blur-AutoClicker).

## Prerequisites

- macOS 12 or newer on Apple Silicon
- Xcode Command Line Tools (`xcode-select --install`)
- Node.js 20 or newer
- Rust (`brew install rust`, or via `rustup`)

## Setup

```bash
git clone https://github.com/trwitlin-hash/Blur-AutoClicker-Mac.git
cd Blur-AutoClicker-Mac
npm install
```

## Run in development

```bash
npm run tauri -- dev
```

## Build a release bundle

```bash
APPLE_SIGNING_IDENTITY="-" npm run tauri -- build --target aarch64-apple-darwin
```

`APPLE_SIGNING_IDENTITY` is **required**. Without it Tauri emits a
*linker-signed* bundle: no `Contents/_CodeSignature`, `Sealed Resources=none`,
and `entitlements.plist` is silently ignored, because Tauri only applies
entitlements when it signs. macOS binds the Accessibility (TCC) grant to the
code signature, so an unsealed bundle means the permission will not stick.

Verify before shipping:

```bash
codesign --verify --deep --strict \
  src-tauri/target/aarch64-apple-darwin/release/bundle/macos/BlurAutoClicker.app
```

Outputs land in `src-tauri/target/aarch64-apple-darwin/release/bundle/`
(`macos/BlurAutoClicker.app` and `dmg/*.dmg`).

## Quality gate

```bash
npm run check
```

Runs `cargo test`, `cargo check`, `clippy`, `cargo fmt --check`, the frontend
tests, eslint, prettier, the frontend build and `npm audit`. It refuses to run
while BlurAutoClicker is open, because a running instance holds the build output.

## Accessibility

Nothing clicks until **System Settings → Privacy & Security → Accessibility**
lists and enables BlurAutoClicker. Every rebuild changes the code signature, so
remove the stale entry with **−** and re-add it after installing a new build.

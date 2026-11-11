# BlurAutoClicker — macOS

A macOS port of [Blur009/Blur-AutoClicker](https://github.com/Blur009/Blur-AutoClicker),
tracking upstream **v3.9.6**.

> **Modification notice (GPL-3.0 §5a).** This is a modified version of
> BlurAutoClicker. The Windows-only input layer was replaced with a macOS
> CoreGraphics implementation, and the build was retargeted to macOS
> (`universal-apple-darwin`: Apple Silicon and Intel). Modified on
> **2026-09-05**, based on upstream tag `v3.9.6`.
>
> The Windows implementation has been **removed entirely**, not just gated off:
> the Win32 input layer, WebView2 bootstrapper, NSIS packaging, PowerShell
> scripts and MSVC toolchain config are all gone. This is a macOS-only tree.
> Use [upstream](https://github.com/Blur009/Blur-AutoClicker) for Windows.

Upstream is Windows-only by design — the author has
[declined to support macOS](https://github.com/Blur009/Blur-AutoClicker/pull/214)
("I do not own a macbook of any kind"). This fork exists to fill that gap.

## Install

Download the `.dmg` from Releases, drag the app to **Applications**, then:

```bash
# ad-hoc signed, so Gatekeeper needs the quarantine flag cleared
xattr -cr /Applications/BlurAutoClicker.app
```

Or right-click the app → **Open** on first launch.

### Accessibility permission is required

macOS silently discards synthetic events from untrusted processes — the app will
run and count clicks while none of them land.

1. **System Settings → Privacy & Security → Accessibility**
2. Enable **BlurAutoClicker**
3. **Quit and relaunch** — the event tap is only created at startup

The grant is bound to the app's code signature, so after any rebuild you must
remove the stale entry with **−** and re-add it.

## What changed from upstream

The table below records what each Windows mechanism was replaced with; the
Windows side no longer exists in this tree.

| Concern | Windows (upstream) | macOS (this fork) |
| --- | --- | --- |
| Click / key injection | `SendInput` | `CGEventPost` + `CGEventCreateMouseEvent` / `…KeyboardEvent` |
| Global hotkey capture | `SetWindowsHookExW` (`WH_KEYBOARD_LL`) | `CGEventTap` on its own CFRunLoop |
| Modifier state | `GetAsyncKeyState` | `CGEventSourceKeyState` — modifiers emit `NX_FLAGSCHANGED`, which the tap never sees |
| Cursor-over-own-window | `WindowFromPoint` + PID compare | Tauri window rect vs. cursor position |
| Key auto-repeat | `SystemParametersInfoW` | `defaults read -g InitialKeyRepeat` / `KeyRepeat`, cached |
| Function keys | `VK_F1 + (n-1)`, contiguous | lookup table — macOS F-key codes are **not** contiguous |
| Settings / stats path | `%APPDATA%` | `~/Library/Application Support/BlurAutoClicker` |
| Modifier labels | `Ctrl` / `Alt` / `Super` | `⌃` / `⌥` / `⌘` |

Known no-ops on macOS: `disableScreenshots` (relies on `SetWindowDisplayAffinity`)
and Crashpad (no prebuilt macOS artifact). The auto-updater is **not registered on
macOS at all** — upstream's update feed publishes Windows artifacts and is signed
with upstream's key, so leaving it wired up would both fail and hand a third party
replace authority over this fork's installs. Update by downloading a new release.

See [PORTING-NOTES.md](PORTING-NOTES.md) for the full method, including the
three-way merge used and the required post-build re-signing step.

## Building

Needs both targets: `rustup target add aarch64-apple-darwin x86_64-apple-darwin`.

```bash
npm install
npm run tauri -- build --target universal-apple-darwin --bundles app

APP=src-tauri/target/universal-apple-darwin/release/bundle/macos/BlurAutoClicker.app
codesign --force --deep --sign - --entitlements src-tauri/entitlements.plist "$APP"
```

**Sign separately, and do not let Tauri do it.** Passing
`APPLE_SIGNING_IDENTITY` makes Tauri's bundler sign *with hardened runtime*, and
a hardened-runtime binary carrying only an ad-hoc signature has no signing
authority behind it, so macOS will not extend the Accessibility grant that
`CGEventPost` needs. Leaving it unset is no better: Tauri then emits a
linker-signed bundle with no `_CodeSignature` and silently drops
`entitlements.plist`, which breaks the grant too. Both failures are silent and
only show up as "clicking does nothing".

Verify all three properties before shipping:

```bash
codesign --verify --deep --strict "$APP"
codesign -d --verbose=2 "$APP" 2>&1 | grep -E 'flags=0x2\(adhoc\)|Sealed Resources'
lipo -archs "$APP/Contents/MacOS/BlurAutoClicker"   # expect: x86_64 arm64
```

Then package the signed bundle (`.github/workflows/release.yml` does exactly
this, with the checks as hard failures).

## Credits

- [Blur009](https://github.com/Blur009/Blur-AutoClicker) — the original application
- [Djozman/Blur-AutoClickerMAC](https://github.com/Djozman/Blur-AutoClickerMAC) —
  the 3.9.1 macOS port used as the reference for the CoreGraphics layer

## License

GPL-3.0, inherited from upstream. See [LICENSE](LICENSE).

# BlurAutoClicker — macOS (Apple Silicon)

A macOS port of [Blur009/Blur-AutoClicker](https://github.com/Blur009/Blur-AutoClicker),
tracking upstream **v3.9.6**.

> **Modification notice (GPL-3.0 §5a).** This is a modified version of
> BlurAutoClicker. The Windows-only input layer was replaced with a macOS
> CoreGraphics implementation, and the build was retargeted to
> `aarch64-apple-darwin`. Modified on **2026-09-05**, based on upstream tag
> `v3.9.6`.
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

```bash
npm install
APPLE_SIGNING_IDENTITY="-" npm run tauri -- build --target aarch64-apple-darwin
```

`APPLE_SIGNING_IDENTITY="-"` matters. Without it Tauri emits a *linker-signed*
bundle with no `_CodeSignature` and never applies `entitlements.plist`, so
`codesign --verify` fails and the Accessibility grant will not stick. With it the
bundle is ad-hoc signed with hardened runtime, sealed resources and the
entitlements embedded — verify with:

```bash
codesign --verify --deep --strict src-tauri/target/aarch64-apple-darwin/release/bundle/macos/BlurAutoClicker.app
```

## Credits

- [Blur009](https://github.com/Blur009/Blur-AutoClicker) — the original application
- [Djozman/Blur-AutoClickerMAC](https://github.com/Djozman/Blur-AutoClickerMAC) —
  the 3.9.1 macOS port used as the reference for the CoreGraphics layer

## License

GPL-3.0, inherited from upstream. See [LICENSE](LICENSE).

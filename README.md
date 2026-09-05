# BlurAutoClicker — macOS (Apple Silicon)

A macOS port of [Blur009/Blur-AutoClicker](https://github.com/Blur009/Blur-AutoClicker),
tracking upstream **v3.9.6**.

> **Modification notice (GPL-3.0 §5a).** This is a modified version of
> BlurAutoClicker. The Windows-only input layer was replaced with a macOS
> CoreGraphics implementation, and the build was retargeted to
> `aarch64-apple-darwin`. Modified on **2026-09-05**, based on upstream tag
> `v3.9.6`. Upstream is unmodified in all other respects, and all Windows code
> is preserved behind `#[cfg(target_os = "windows")]`.

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

Known no-ops on macOS: `disableScreenshots` (relies on `SetWindowDisplayAffinity`),
Crashpad (no prebuilt macOS artifact), and the auto-updater (upstream publishes no
macOS artifact, so its check is inert by design rather than offering a Windows
installer).

See [PORTING-NOTES.md](PORTING-NOTES.md) for the full method, including the
three-way merge used and the required post-build re-signing step.

## Building

```bash
npm install
npm run tauri -- build --target aarch64-apple-darwin
```

Tauri ad-hoc signs the binary but does not seal the bundle, so re-sign afterwards:

```bash
codesign --force --deep --sign - \
  --entitlements src-tauri/entitlements.plist \
  /Applications/BlurAutoClicker.app
```

## Credits

- [Blur009](https://github.com/Blur009/Blur-AutoClicker) — the original application
- [Djozman/Blur-AutoClickerMAC](https://github.com/Djozman/Blur-AutoClickerMAC) —
  the 3.9.1 macOS port used as the reference for the CoreGraphics layer

## License

GPL-3.0, inherited from upstream. See [LICENSE](LICENSE).

# BlurAutoClicker 3.9.6 — macOS (Apple Silicon) port

## What this is

A native macOS build of BlurAutoClicker **v3.9.6** — the same version as the
Windows portable copy in `~/Downloads/BlurAutoClicker-v3.9.6-portable`. It is
not an emulator, a wrapper, or a lookalike: it is the upstream Rust + React
source compiled for `aarch64-apple-darwin`, with the Windows-only input layer
replaced by the macOS equivalent.

Upstream is GPL-3.0, which explicitly permits this.

## Why the .exe itself can't run

`BlurAutoClicker.exe` is a PE32+ binary (`MZ` header) for Windows x86-64.
macOS has no loader for that format. Beyond that, it is a **Tauri** app, so it
needs Microsoft Edge WebView2, which does not exist on macOS. And even under
Wine or in a Windows VM, an autoclicker's synthetic clicks stay inside that
sandbox — they never reach real macOS apps. Porting was the only route that
actually delivers the functionality.

## Provenance

| Source | Role |
| --- | --- |
| `Blur009/Blur-AutoClicker` @ `v3.9.6` | upstream source of truth (GPL-3.0) |
| `Djozman/Blur-AutoClickerMAC` @ 3.9.1 | reference for the CoreGraphics layer |

The upstream author has declined to support macOS ("I do not own a macbook of
any kind... I will not be supporting this officially") and closed five
community port PRs. The Djozman fork is a genuine port but is pinned at
**3.9.1**. This build merges that port forward onto **3.9.6**.

Method: a three-way merge with base = upstream `v3.9.1`, ours = upstream
`v3.9.6`, theirs = the fork. 13 files merged clean; 8 conflicted and were
resolved by hand.

## Windows → macOS mapping

| Concern | Windows (upstream) | macOS (this build) |
| --- | --- | --- |
| Click / key injection | `SendInput` | `CGEventPost` + `CGEventCreateMouseEvent` / `…KeyboardEvent` |
| Global hotkey capture | `SetWindowsHookExW` (`WH_KEYBOARD_LL`) | `CGEventTap` on its own CFRunLoop |
| Modifier state | `GetAsyncKeyState` | `CGEventSourceKeyState` (modifiers emit `NX_FLAGSCHANGED`, which the tap never sees) |
| Cursor-over-own-window | `WindowFromPoint` + PID compare | Tauri window rect vs. cursor position |
| Key auto-repeat | `SystemParametersInfoW` | `defaults read -g InitialKeyRepeat` / `KeyRepeat`, cached |
| Function keys | `VK_F1 + (n-1)`, contiguous | lookup table — macOS F-key codes are **not** contiguous |
| Stats/settings path | `%APPDATA%` | `~/Library/Application Support/BlurAutoClicker` |
| Taskbar icon (`HICON`) | `CreateIconIndirect` | not applicable, compiled out |

All Windows code is preserved behind `#[cfg(target_os = "windows")]` rather
than deleted, so the tree can still target Windows.

## Deliberate differences

- **Auto-update is inert.** Upstream's `latest.json` has no `darwin` platform
  key, so the updater's `check()` is a clean no-op. The in-app "check for
  updates" (a plain GitHub API call) still works and will report new releases —
  but those releases are Windows-only. To move to a newer version, re-run the
  build script against the new tag.
- **Crash reporting is off.** `crashpad-rs`'s prebuilt artifact has no macOS
  build, so the `crashpad` Cargo feature is no longer on by default.
- **`disableScreenshots` does nothing.** It relies on
  `SetWindowDisplayAffinity`, which has no macOS equivalent.
- **Autostart** uses the macOS launch-agent path, not `HKCU\Run`.
- The Windows path inside `click_point_picker.rs` follows the fork's 3.9.1
  lineage rather than 3.9.6's (a two-line reordering). It is `cfg`-stripped on
  macOS and so has no effect on this build.

## Accessibility permission — required

macOS will not let **any** process post synthetic events to other apps without
it. Until it is granted, the app runs but no clicks land.

1. Launch BlurAutoClicker.
2. **System Settings → Privacy & Security → Accessibility**
3. Enable **BlurAutoClicker**.

Note: the permission is bound to the binary's code signature. This build is
ad-hoc signed, so **if you rebuild it, remove the old entry with `−` and re-add
it** — otherwise macOS silently keeps refusing.

## Gatekeeper

The app is not Developer-ID signed or notarized (that needs a paid Apple
Developer account). First launch: **right-click → Open**, or clear the quarantine
flag:

```bash
xattr -cr /Applications/BlurAutoClicker.app
```

## Rebuilding

```bash
cd ~/Downloads/BlurAutoClicker-mac/upstream && npm run tauri -- build --target aarch64-apple-darwin
```

### Important: re-sign the bundle after every build

Tauri ad-hoc signs the *binary* but does not seal the *bundle* — the built
`.app` has no `Contents/_CodeSignature`, and `codesign --verify` fails with
"code has no resources but signature indicates they must be present".

Because macOS binds the Accessibility (TCC) grant to the code signature, an
unsealed bundle can cause the permission to fail to stick. Seal it after each
build:

```bash
codesign --force --deep --sign - \
  --entitlements ~/Downloads/BlurAutoClicker-mac/upstream/src-tauri/entitlements.plist \
  /Applications/BlurAutoClicker.app
```

Then confirm: `codesign --verify --deep --strict /Applications/BlurAutoClicker.app`
should report nothing, and `codesign -dv` should show `Sealed Resources`.

Re-signing changes the cdhash, so after any rebuild remove the old
Accessibility entry with `-` and re-add it.

## One caveat worth stating plainly

If the target is a multiplayer game, automated clicking is against the rules of
many servers, and detection specifically looks for consistent click intervals.
That is exactly what this app's speed-randomization feature exists to soften,
but it does not eliminate the risk. Single-player has no such issue.

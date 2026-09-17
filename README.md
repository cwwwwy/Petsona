# Petsona

A lightweight, Rust-only desktop pet for Windows and macOS. No Node, no WebView,
no Tauri — native windows, a Codex-compatible pet engine and an optional
DeepSeek greeting.

## Features

- Bundled Codex pet (**Superintendent** by Renner Campos) plus a local,
  user-owned pet library
- 8×9 / 8×11 Codex pet packs, animation state machine, pixel-accurate click-through
- Click / double-click / drag / native context menu / cursor gaze / speech bubble
- Explicit import from Codex (`~/.codex/pets`), a folder or a `.zip`
- Local state protocol (`127.0.0.1:17872`) so Codex hooks can drive the pet
- Persona plus lightweight JSON memory (facts, recent events, last seen)
- Windows: tray, native Win32 menu, login autostart, multi-monitor position
  memory, gravity
- macOS: AppKit shell, tray menu, LaunchAgent autostart script

## Workspace

| Crate | Contents |
|---|---|
| `crates/petsona-core/` | Pet format, animation engine, persona, memory, DeepSeek, state protocol |
| `crates/petsona-runtime/` | Platform-independent runtime: config, sessions, locks, logs, greetings |
| `crates/petsona-app/` | Shared egui UI + `PlatformHost` boundary (library, no binary) |
| `crates/petsona-shell-windows/` | Win32 shell → `petsona-windows.exe` |
| `crates/petsona-shell-macos/` | AppKit shell → `petsona-macos` |
| `legacy/` | Old Tauri app, reference only |

Platform code lives in the shells, not in `petsona-app`: each shell implements
`petsona_app::platform::PlatformHost` and calls `petsona_app::run(host)`.

## Run

```powershell
cargo run -p petsona-shell-windows   # Windows
cargo run -p petsona-shell-macos     # macOS
```

Windows needs VS Build Tools ("Desktop development with C++") for the MSVC
linker. A GNU toolchain fallback is documented in `docs/WINDOWS_VERIFICATION.md`.

## Verify

```text
macOS    bash scripts/verify-macos-all.sh               # gates + runtime smoke + package smoke
         bash scripts/verify-macos-all.sh --gates-only  # fmt / clippy / test / release
Windows  powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1        # gates
         powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1 -Full  # gates + smoke
```

Window, tray, menu and click-through behaviour still needs the manual checklists
in `docs/WINDOWS_VERIFICATION.md` / `docs/MACOS_VERIFICATION.md`.

## Package

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package-windows.ps1   # dist\Petsona-windows-x64-<version>.zip
bash scripts/package-macos.sh                                          # dist/Petsona.app + zip
```

The Windows exe embeds `packaging/windows/Petsona.ico`, generated from the macOS
artwork by `packaging/windows/generate-icon.ps1`.

## State protocol

```bash
curl -XPOST http://127.0.0.1:17872/state \
  -H 'content-type: application/json' \
  -d '{"source":"codex","state":"running","message":"running tests","ttlMs":120000}'
```

`GET /health` returns the current pet/persona/state snapshot, `GET /pets` lists the
pet ids. Available states: `idle`, `running`, `waiting`, `failed`, `review`,
`waving`, `jumping`, `running-left`, `running-right`, `look-row-9`, `look-row-10`.
`ttlMs: 0` means "never expires".

## Pets

The app loads only its own library (`<config>/Petsona/pets`). Codex pets are
copied there explicitly via Settings → 宠物 → 「从 Codex 导入」; importing a
folder or `.zip` (or dropping it on the window) works too. Packages are
validated before import, exports use the Codex upload format, and deleting a
local copy asks for confirmation. The bundled Superintendent is installed on
every start unless you delete it in Settings.

> The bundled pet has no license field in `pet.json`; confirm the author's terms
> before distributing a release.

Inspect any pet package:

```powershell
cargo run -p petsona-core --example pet_inspect -- "$env:USERPROFILE\.codex\pets\boba" .scratch\boba
```

## DeepSeek (optional)

```text
base URL: https://api.deepseek.com/v1
model:    deepseek-v4-flash
```

```powershell
$env:DEEPSEEK_API_KEY = "sk-..."
```

The settings window can also save the key to the OS keychain. Greetings are
non-streaming and use a small token budget; without a key Petsona falls back to
the persona's fixed or time-based greeting.

## Docs

| File | Contents |
|---|---|
| `docs/PLATFORM_ARCHITECTURE.md` | Shared core + platform shells, menu decision, release tracks |
| `docs/WINDOWS_VERIFICATION.md` | Windows manual checklist (includes autostart, multi-monitor, gravity) |
| `docs/MACOS_VERIFICATION.md` | macOS manual checklist |
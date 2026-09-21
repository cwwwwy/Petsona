# Petsona

A lightweight desktop pet for Windows and macOS. A shared Rust core/runtime drives
native platform frontends (Windows: C# / WinUI 3 + Win32, macOS: SwiftUI / AppKit),
with a Codex-compatible pet engine and an optional DeepSeek greeting.

## Features

- Local, user-owned pet library (no bundled pet; pick or import one on first run)
- 8×9 / 8×11 Codex pet packs, animation state machine, pixel-accurate click-through
- Click / double-click / drag / native context menu / cursor gaze / speech bubble
- Explicit import from Codex (`~/.codex/pets`), a folder or a `.zip`
- macOS settings: DeepSeek configuration, persona CRUD/templates, JSON memory
  facts/events, drag-and-drop import with overwrite confirmation, fixed scale presets
- Local state protocol (`127.0.0.1:17872`) so Codex hooks can drive the pet
- Persona plus lightweight JSON memory (facts, recent events, last seen)
- Windows: WinUI 3 + Win32 frontend — tray with native menu, login autostart,
  single-monitor position memory (multi-monitor / gravity / activity reminders are
  deferred, see [plan §3.1](docs/plans/windows-native-rewrite.md))
- macOS: SwiftUI / AppKit frontend — menu-bar menu, LaunchAgent autostart

## Workspace

| Crate | Contents |
|---|---|
| `crates/petsona-core/` | Pet format, animation engine, persona, memory, DeepSeek, state protocol |
| `crates/petsona-runtime/` | Platform-independent runtime: config, sessions, locks, logs, greetings |
| `crates/petsona-ffi/` | Native frontend C ABI; ABI 3 contract and worker projection |
| `apps/windows/` | C# / WinUI 3 + Win32 native Windows frontend (current product line) |
| `apps/macos/` | SwiftUI + AppKit native macOS frontend (manual acceptance in progress) |
| `crates/petsona-app/` | Legacy egui UI, frozen — deleted after the macOS line is accepted |
| `crates/petsona-shell-windows/` | Legacy Rust/Win32 shell, frozen — kept buildable only |
| `crates/petsona-shell-macos/` | Legacy Rust/AppKit shell, frozen — kept buildable only |

Both native frontends call the shared engine through `contracts/petsona.h`. The
old egui / shell crates stay in the workspace only until the macOS line finishes
manual acceptance (then they are deleted, see Windows plan REQ-W15).

Windows state: the native frontend passed its manual acceptance matrix (W1–W14)
and the automated gate; packaging (`scripts\package-windows.ps1`) and the release
workflow still need their first runs on a clean machine.
macOS state: functional batch + gates green, but window/IME/Keychain and signing
acceptance is still open.
See the plans ([macOS](docs/plans/native-ui-rewrite.md),
[Windows](docs/plans/windows-native-rewrite.md)), their
[execution records](docs/execution/), and [collaboration rules](AGENTS.md).

## Run

```powershell
# Windows (current): build or run the native frontend
dotnet build apps/windows/Petsona.sln -c Release -p:Platform=x64
powershell -ExecutionPolicy Bypass -File scripts\package-windows.ps1   # then run dist\…\Petsona.exe

# macOS (current)
xcodegen generate --spec apps/macos/project.yml --project apps/macos
xcodebuild -project apps/macos/Petsona.xcodeproj -scheme Petsona build

# Legacy entries (frozen, kept buildable for comparison only)
cargo run -p petsona-shell-windows
cargo run -p petsona-shell-macos
```

Windows needs VS Build Tools ("Desktop development with C++") for the MSVC
linker. A GNU toolchain fallback is documented in `docs/WINDOWS_VERIFICATION.md`.

## Verify

```text
macOS    bash scripts/verify-macos-all.sh               # Rust + native build + smoke + package
         bash scripts/verify-macos-all.sh --gates-only  # Rust + native build gates
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
`ttlMs: 0` means "never expires"; the source that raised a state can retract it
again (e.g. after a crash) without naming a new state:

```bash
curl -XPOST http://127.0.0.1:17872/state \
  -H 'content-type: application/json' -d '{"source":"codex","action":"clear"}'
```

## Pets

The app loads only its own library (`<config>/Petsona/pets`). Codex pets are
copied there explicitly via Settings → 宠物 → 「从 Codex 导入」; importing a
folder or `.zip` (or dropping it on the window) works too. Packages are
validated before import, exports use the Codex upload format, and deleting a
local copy asks for confirmation.

Petsona ships without a bundled pet: on first launch (or whenever the local
library is empty) the Settings window opens so a pet can be picked from
`~/.codex/pets` or imported from a folder / `.zip`.

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
| `docs/WINDOWS_VERIFICATION.md` | Windows native manual checklist (W/D/F, includes the W11 protocol steps) |
| `docs/MACOS_VERIFICATION.md` | macOS manual checklist |
| `docs/plans/` · `docs/execution/` | Cross-conversation plan contracts and evidence records |
| `docs/archive/` | Frozen history: legacy egui Windows checklist and the old Windows issue tracker |

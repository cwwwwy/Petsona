# Petsona

Petsona is a lightweight, Rust-only desktop pet that uses AI to cultivate a
personality and keep the user company.

The new application keeps the parts that made the original Codex pet useful:

- reads Codex-compatible pet packages from `~/.codex/pets`
- renders the 8x9 / 8x11 spritesheet animation state machine
- supports a transparent, always-on-top pet window
- supports click, double-click, drag, right-click menu, gaze and a speech bubble
- can switch pets at runtime from `~/.codex/pets`, `~/.unipet/pets` or the local library
- exposes the local state protocol so Codex hooks can drive the animation
- stores a simplified persona and lightweight pet memory
- uses one DeepSeek API transport for short, intelligent greetings

The exact frame inventory of the shipped Codex pet and every replicated
behaviour is documented in [docs/PET_NATIVE.md](docs/PET_NATIVE.md).

The previous Tauri + WebView application is kept under `legacy/` as a reference
and is no longer part of the workspace.

## Workspace

```text
crates/petsona-core/          Pet format, animation engine, persona, memory, DeepSeek client
crates/petsona-runtime/       Platform-independent runtime: config, memory, protocol, locks, logs
crates/petsona-app/           Shared egui UI and the platform::PlatformHost boundary (library)
crates/petsona-shell-windows/ Win32 shell (binary: petsona-windows)
crates/petsona-shell-macos/   AppKit shell (binary: petsona-macos)
legacy/                       Previous Tauri app and frontend, reference only
```

Platform code no longer lives in `petsona-app`: each shell implements
`petsona_app::platform::PlatformHost` and calls `petsona_app::run(host)`.

## Build

```powershell
cargo run -p petsona-shell-windows   # Windows
cargo run -p petsona-shell-macos     # macOS
```

The Windows MSVC target still requires the MSVC linker. Install Visual Studio
Build Tools with the "Desktop development with C++" workload before building.

## Verify

One entry point per platform, run from the repository root:

```text
macOS:   bash scripts/verify-macos-all.sh               # gates + runtime smoke + package smoke
         bash scripts/verify-macos-all.sh --gates-only  # fmt / clippy / test / release build
Windows: powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1        # gates
         powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1 -Full  # gates + smoke
```

`scripts/macos-smoke.sh` is the runtime smoke implementation; it stays a separate
file because it is long and can be rerun against an already-built bundle. The
package-structure check runs inline in the macOS entry point. Native window interactions still require the manual checks in
`docs/MACOS_VERIFICATION.md`.

## Package

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package-windows.ps1   # dist\Petsona-windows-x64-<version>.zip
bash scripts/package-macos.sh                                          # dist/Petsona.app + zip
```

The Windows executable embeds `packaging/windows/Petsona.ico` (built from the
macOS artwork by `packaging/windows/generate-icon.ps1`); the macOS bundle is
produced by the matching script with the icon, `Info.plist` and `LSUIElement`.

## Interactions

| Input | Behaviour |
|---|---|
| Left click | wave + a greeting bubble (DeepSeek when a key is configured, else the persona's fallback greeting) |
| Double click | jump |
| Drag | move the pet |
| Right click | menu: open settings / close pet |
| Cursor at either side | the V2 look row turns and holds its gaze while the cursor stays there |
| Tray icon | open settings / show-hide pet / quit |
| Every 45 min | activity reminder: the pet walks a short distance and asks you to stand up |

## Inspecting a pet

```powershell
cargo run -p petsona-core --example pet_inspect -- "$env:USERPROFILE\.codex\pets\boba" .scratch\boba
```

Prints the resolved grid, how many frames each row actually draws and the
animation table the engine builds; the optional output directory receives one
PNG per animation row.

## State protocol

Petsona listens on `127.0.0.1:17872` (settings -> 状态协议) so hooks and scripts
can drive the pet:

```bash
curl -XPOST http://127.0.0.1:17872/state \
  -H 'content-type: application/json' \
  -d '{"source":"codex","state":"running","message":"正在跑测试","ttlMs":120000}'
```

`GET /health` returns the current pet/persona/state snapshot and `GET /pets`
lists the discovered pets.

## Pet library

The application keeps its own writable library next to the config file
(`<config>/Petsona/pets` on every platform) and links `~/.codex/pets` and
`~/.unipet/pets` read-only, with the local library winning on id clashes.

The bundled ByteBot is installed into the local library on every start, so it
is always available to switch back to; deleting it in the settings opts out for
good. Settings -> 宠物 imports a pet folder or `.zip` (also by dropping it onto
the window), exports the Codex upload format, and deletes local copies with a
confirmation. Packages are validated before import: manifest, geometry,
decoding, path traversal, size and entry limits.

## DeepSeek

The first version uses the OpenAI-compatible Chat Completions endpoint:

```text
base URL: https://api.deepseek.com/v1
model:    deepseek-v4-flash
```

Set the API key in the environment:

```powershell
$env:DEEPSEEK_API_KEY = "sk-..."
cargo run -p petsona-shell-windows
```

The settings window can also save the key to the operating system keychain.
Greeting requests are non-streaming and use a small token budget; if the API is
unavailable, Petsona falls back to the persona's fixed or time-based greeting.

## Memory

Memory is stored in `memory.json` under the platform config directory. It is
not a chat transcript. It contains only:

- long-term facts
- recent interaction events
- last-seen and last-greeting state

This keeps the rebuilt application small while still allowing greetings to
refer to stable preferences and recent context.

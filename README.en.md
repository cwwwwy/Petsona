# Petsona

[English](README.en.md) | [简体中文](README.md)

A desktop pet and AI companion for Windows and macOS. Petsona brings an animated companion to your desktop, with chat, conversation history, character settings, and natural-language memories.

Version 0.1.0 is in release preparation. Desktop features and M6-A cleanup are complete, and the Git history has been rebuilt as a single initial commit. Installers, signing, notarization, and install/upgrade acceptance are still pending.

## Features

- Import pet packs from folders or ZIP files, or bring pets in from the Codex pet library. Petsona does not include a built-in pet.
- Shape a character with a name and a visible description. Create a draft from source material and try it in chat.
- Review, correct, or forget long-term memories. Chat history and long-term memory are managed separately.
- Use the native desktop pet, speech bubbles, and composer alongside WebView-based settings and chat pages.

## Data and privacy

- Windows data: %APPDATA%\Petsona.
- macOS data: ~/Library/Application Support/Petsona. Minimum supported macOS version: 26.
- Logs are stored in the data directory at logs/petsona.log and logs/shell.log. Each file is limited to 4 MiB with one rotated backup.
- Model credentials are stored in the operating system credential store, not in configuration files or logs.
- The local state protocol listens on 127.0.0.1:17872 and provides POST /state, GET /health, and GET /pets.

## Architecture

Rust owns application state, pet animation, characters, memories, sessions, native windows, and the local protocol. The Tauri shell handles platform integration and native overlays. The React/TypeScript frontend provides settings and chat content pages.

Pets, bubbles, and the composer are rendered in native Rust windows. WebView renders content pages only. The project maintains one Rust/TypeScript desktop implementation and no longer includes the legacy C#/Swift frontends or C ABI.

## Development

Rust is pinned to 1.98.0. Frontend development requires Node.js and pnpm. macOS builds require Xcode; Windows builds use the pinned MSVC toolchain and Visual Studio Build Tools.

~~~sh
pnpm --dir apps/desktop install --frozen-lockfile
pnpm --dir apps/desktop build
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
~~~

State protocol tests bind a local loopback port. Run them in an environment that permits local networking. Tests must use isolated data directories and ports.

## Build and verify

Acceptance builds include native verification hooks, automatic exit, and diagnostic captures. Formal builds omit those acceptance-only hooks. Both modes use separate output directories.

macOS:

~~~sh
bash scripts/desktop-build-macos.sh debug
python3 scripts/desktop-verify-macos.py
bash scripts/desktop-build-macos.sh release formal
~~~

Windows from WSL (build and verify one profile):

~~~sh
petsona_repo_win="$(wslpath -w "$PWD")"
pnpm --dir apps/desktop build
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$petsona_repo_win\scripts\desktop-build-windows.ps1" -RepoRoot "$petsona_repo_win" -Profile release
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$petsona_repo_win\scripts\desktop-verify-windows.ps1" -RepoRoot "$petsona_repo_win" -Profile release
~~~

Automated regression uses isolated app processes, windows, data, credentials, and ports. It does not move the mouse or synthesize keyboard or wheel input; pointer interactions, IME, tray behavior, and visual acceptance are manual.

The local macOS app is unsigned and is not a distributable installer.

## Release status

The first installers, Windows runtime distribution, macOS signing/notarization, clean-machine checks, and install/upgrade/uninstall acceptance are planned for M6-B.

## Development documents

The long-term development roadmap and stage-by-stage execution evidence are maintained in the [development plan](docs/DEVELOPMENT_PLAN.md) and [development execution record](docs/DEVELOPMENT_EXECUTION.md). Architecture and collaboration details live in [AGENTS.md](AGENTS.md).

## License

MIT

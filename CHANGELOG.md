# Changelog

All notable changes to Petsona are documented here. Release tags are
`windows-v<version>` / `macos-v<version>`; the version comes from the workspace
`version` in `Cargo.toml`.

## [0.1.0-rc.1] - 2026-09-22 (Windows pre-release)

First public Windows build. Portable zip, **self-contained** (bundles the .NET
runtime and the Windows App SDK, so nothing has to be installed first).

### Added

- Native Windows frontend (WinUI 3 + Win32): transparent, frameless, always-on-top
  pet window with pixel-accurate click-through, drag/click/double-click, cursor
  gaze (official 22.5° 16-direction mapping with hysteresis) and tray menu.
- Codex pet packs: 8×9 / 8×11 atlases (webp first-class), local library with
  import from folder / `.zip` / `~/.codex/pets`, export, delete and overwrite
  confirmation. No bundled pet — the first launch opens Settings.
- Settings as six instant-apply cards: 宠物 / 外观与交互 / 说话方式 / 记忆 /
  模型服务 / 系统 (icons, responsive icon rail).
- Speaking style per pet: switching pets switches tone, emoji and system prompt;
  five tone presets plus free-form editing, import/export, reset to built-in.
- Memory per pet: recent events, editable facts with **source labels**
  (manual / conversation / import / compressed), tiered clears, export/import,
  event retention in days and automatic fact compression into a 「画像」 fact.
- Model service: DeepSeek preset or any OpenAI-compatible endpoint, model list
  fetched from the provider (with manual fallback), per-provider credentials in
  the Windows Credential Manager, DeepSeek-only `thinking` field gated by provider.
- Idle greetings: after N idle minutes the pet says a short line (model-generated,
  falling back to the persona's fixed greeting) as a bubble.
- Local state protocol on `127.0.0.1:17872` (`POST /state`, `GET /health`,
  `GET /pets`) plus `{"source":"x","action":"clear"}` to retract a sticky state.

### Known limitations

- Unsigned binaries: Windows SmartScreen shows "unknown publisher" on first run.
- The zip is ~100 MB: it bundles the .NET runtime and Windows App SDK.
- Deferred by design (tracked in `docs/plans/windows-native-rewrite.md` §3.1):
  multi-monitor placement, gravity, activity reminders, opacity, protocol UI,
  pet-icon tray mode and shadow animation.
- macOS build is tracked separately (`macos-v*`) and still needs on-device
  acceptance.

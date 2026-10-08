# Changelog

All notable changes to Petsona are documented here. Release tags are
`windows-v<version>` / `macos-v<version>`; the version comes from the workspace
`version` in `Cargo.toml`.

## [Unreleased]

### Changed

- 2026-10-08：启动桌面壳重建（`desktop-shell-rust-ts`）——确定 Rust（core/runtime/平台层/浮层）+ TypeScript
  （全部内容界面）架构；旧 C#/WinUI 与 Swift/AppKit 前端、`petsona-ffi`（C ABI）、旧脚本与旧文档自工作树删除，
  完整快照保留在 git `6bca241`。
- 行为规格与产品决策迁移：人工验收矩阵 → `docs/DESKTOP_VERIFICATION.md`；设置/人格/记忆/模型/问候等决策 →
  `docs/plans/desktop-shell-rust-ts.md`「继承的产品决策」。
- 保留的 `petsona-core` / `petsona-runtime` 门禁通过：114/114（core 86 / runtime 28）。
- 旧线 `windows-v0.1.0-rc.1` 未发布，作废；新壳首个正式版目标为 `windows-v0.1.0`。

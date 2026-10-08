# Petsona

Windows / macOS 桌宠与 AI 伴侣：读取 Codex 宠物包（`pet.json` + 图集），按锁定基准播放动画，
支持流式聊天、本地历史、人格与记忆。

> **状态：桌面壳重建中（2026-10-08 起）。**
> 项目正按 [desktop-shell-rust-ts 计划](docs/plans/desktop-shell-rust-ts.md) 重建为
> **Rust**（core / runtime / 平台层 / 浮层窗口）+ **TypeScript**（设置、聊天、历史、人格、宠物库等全部内容界面）。
> 旧 C#/WinUI 与 Swift/AppKit 前端已删除，完整快照在 git `6bca241`；
> 当前保留并持续通过测试的代码是 `crates/petsona-core` 与 `crates/petsona-runtime`，
> 新产品入口 `apps/desktop` 处于 M0 建设期。

## 目标架构

```text
crates/petsona-core      宠物格式、动画与注视、人格、记忆、DeepSeek、状态协议
crates/petsona-runtime   配置与会话、宠物库、实例锁、日志、问候、流式聊天
        │  同进程 crate 调用（无 C ABI）
apps/desktop/src-tauri   Tauri 2 壳：托盘、窗口几何、凭据、自启 + 原生浮层渲染
        │  Tauri commands / events（JSON 投影）
apps/desktop/src         TypeScript 内容界面（设置六页、聊天、历史、人格、宠物库）
```

设计铁律：**宠物、气泡、编辑条、Composer 由 Rust 原生窗口渲染；WebView 只承载内容页面。**

## 开发

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace            # core + runtime；协议测试需要本地回环端口
```

`apps/desktop` 的构建与验收命令在 M0 建立后补充到
[docs/DESKTOP_VERIFICATION.md](docs/DESKTOP_VERIFICATION.md)。

测试与验收始终使用隔离数据目录（`PETSONA_HOME`）、独立端口与假凭据，不影响日常实例。

## 数据与协议

- 数据目录：`%APPDATA%\Petsona`（Windows）/ `~/Library/Application Support/Petsona`（macOS）。
- 宠物库：本地导入的 `pets/`；`~/.codex/pets` 仅作为「从 Codex 导入」来源；无内置宠物。
- 状态协议：`127.0.0.1:17872` 的 `POST /state`、`GET /health`、`GET /pets`（语义与手工命令见验收清单附录）。

## 文档

| 文档 | 内容 |
|---|---|
| [平台架构](docs/PLATFORM_ARCHITECTURE.md) | 分层、线程模型、接口与数据边界 |
| [桌面壳计划](docs/plans/desktop-shell-rust-ts.md) | P0 清场、M0 验证、M1–M6 里程碑与继承的产品决策 |
| [执行记录](docs/execution/desktop-shell-rust-ts.md) | 每批改动的证据与接续点 |
| [验收清单](docs/DESKTOP_VERIFICATION.md) | M0 决策门 + 浮层/托盘/设置/聊天/协议/macOS 人工矩阵 |

旧实现（C#/WinUI、Swift/AppKit、旧 egui）与旧验收记录已从工作树移除，历史可用 `git show 6bca241:<path>` 查阅。

## License

MIT

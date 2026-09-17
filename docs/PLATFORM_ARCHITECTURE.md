# Petsona 平台架构

## 决策

Petsona 采用「共享核心 + 独立平台外壳」架构：

- `petsona-core` 和 `petsona-runtime` 是共享层。
- Windows 和 macOS 可以各自选择合适的 UI、窗口、菜单、输入和打包实现。
- 两边 UI 不强制共用同一套壳，也不要求发布节奏完全同步。
- Windows 是当前主要开发目标，macOS 保持持续维护；目标投入比例约为 70% / 30%。
- 仍然保持一个仓库、一个 workspace、一个共享核心主线。

## 目标结构

```text
crates/
  petsona-core/          宠物格式、动画、人格、记忆、协议、配置
  petsona-runtime/       平台无关运行时、日志、锁、问候、宠物会话
  petsona-app/           共享 egui UI + `platform::PlatformHost` 边界（纯库）
  petsona-shell-windows/ Win32 外壳（bin `petsona-windows`）
  petsona-shell-macos/   AppKit 外壳（bin `petsona-macos`）
```

外壳通过实现 `PlatformHost` 注入平台能力，由 `petsona_app::run(host)` 启动共享 UI：

```rust
fn main() -> petsona_app::RunResult {
    petsona_app::run(std::sync::Arc::new(WindowsHost::new()))
}
```

`petsona-app` 中不再有任何 `#[cfg(target_os = ...)]`，也不提供二进制；`PlatformHost`
的每个方法都有可移植默认实现（winit 几何、egui 菜单、无全局钩子），因此缺少某个平台
能力时会自动退化为框架行为，而不是编译失败。

## 共享边界

进入 `petsona-core` 或 `petsona-runtime` 的内容必须不依赖具体 UI 框架：

- 宠物状态机和动画时间计算
- 状态协议和 TTL
- 配置、人格、记忆、对话业务模型
- 平台无关的窗口位置、缩放和布局数据
- 日志、实例锁、问候等基础设施

## 平台外壳边界

平台外壳负责：

- 窗口创建、激活策略、透明和 DPI
- 托盘、菜单和系统主题
- 全局光标、鼠标按键和快捷键
- 渲染后端、动画帧调度和点击穿透
- 平台打包、签名、更新和自启

Windows 和 macOS 的外壳可以在行为上不同，但应尽量提供同样的核心能力。

## 发布轨道

两边可以独立发版：

```text
windows-v0.3.0
macos-v0.2.1
```

独立发布工作流已经落地：

```text
.github/workflows/release-windows.yml   tag windows-v*
.github/workflows/release-macos.yml     tag macos-v*（兼容旧的 v*）
```

Windows 侧产物是 `dist\Petsona-windows-<arch>-<version>.zip`（exe 已内嵌图标），
macOS 侧是 `dist\Petsona.app` 与 `Petsona-macos-<arch>.zip`。

共享核心发生变更时，两个平台至少要保证编译和核心测试通过；如果变更触及
平台能力，还应跑对应平台的完整 smoke。

## 分支策略

- `main` 是共享集成主线，日常直接在其上开发。
- 必要时可创建 `codex/win-*` / `codex/mac-*` 短期分支；不长期维护平台分支。
- Git 暂存、提交、推送和分支操作由用户执行，详见 `AGENTS.md`。

## 迁移进度

截至 2026-09-17，共享运行时、共享 UI 边界与两端平台外壳拆分均已完成：

- `petsona-runtime` 承担配置、人格、记忆、宠物库、会话和平台无关基础设施。
- `petsona-app` 是纯共享 UI 库，通过 `run(host)` 接收平台能力，不再包含平台条件编译。
- 平台后端位于 `petsona-shell-windows` / `petsona-shell-macos`；`petsona-app/src/platform.rs`
  只保留 trait、共享类型和可移植默认实现。
- Windows/macOS 独立打包与 release workflows 已建立；Windows 自启、物理像素位置记忆和重力也已实现。

尚未完成的实机/发布验收以 [`MACOS_VERIFICATION.md`](MACOS_VERIFICATION.md) 和
[`WINDOWS_VERIFICATION.md`](WINDOWS_VERIFICATION.md) 为准；代码结构进度不代表平台人工验收已通过。

## 非目标

- 不拆成两个独立仓库。
- 不复制宠物引擎、协议、人格或记忆逻辑。
- 不为追求两边完全一致而阻止某个平台领先开发。
- 不在第一轮重构中重写现有窗口和渲染后端。

# Petsona 平台架构

## 决策

- 一个仓库、一个 workspace、一个共享核心主线。
- 共享层：`petsona-core` + `petsona-runtime` + `petsona-app`（纯 UI 库）。
- Windows / macOS 各自拥有 UI 外壳，可独立发布（`windows-v*` / `macos-v*`），
  不要求发布节奏一致。
- 投入比例约为 Windows 70% / macOS 30%；Windows 领先不算欠债，但不能破坏共享层。
- 不拆仓库、不拆核心逻辑、不长期维护两套平台主线。

## 结构

```text
crates/
  petsona-core/          宠物格式、动画引擎、人格、记忆、状态协议
  petsona-runtime/       配置、宠物会话、日志、实例锁、问候
  petsona-app/           共享 egui UI + PlatformHost 边界（纯库，无二进制）
  petsona-shell-windows/ Win32 外壳（bin petsona-windows）
  petsona-shell-macos/   AppKit 外壳（bin petsona-macos）
```

外壳实现 `PlatformHost` 后交给共享 UI 启动：

```rust
fn main() -> petsona_app::RunResult {
    petsona_app::run(std::sync::Arc::new(WindowsHost::new()))
}
```

## 边界

| 层 | 负责 |
|---|---|
| `petsona-core` | 宠物状态机与动画时间、状态协议与 TTL、人格 / 记忆 / 对话模型 |
| `petsona-runtime` | 配置读写、宠物库与会话、日志、实例锁、问候调度 |
| `petsona-app` | egui 界面与交互协调、窗口几何计算、`PlatformHost` trait |
| 平台外壳 | 窗口创建 / 激活 / 透明 / DPI、托盘与菜单、全局输入、点击穿透、打包与自启 |

`PlatformHost` 的每个方法都有可移植默认实现（winit 几何、egui 菜单、无全局钩子），
缺少平台能力时自动退化为框架行为。`petsona-app` 里不允许出现 `#[cfg(target_os = ...)]`。

能力分组（完整签名见 `petsona_app::platform::PlatformHost`）：

- 窗口与几何：`present_window`、`set_window_geometry_physical`、`monitor_work_area`、
  `PhysicalRect`（物理像素矩形）。
- 系统集成：菜单（`create_menu`、`PlatformMenu`）、自启（`autostart_*`）、
  文件面板与文件管理器、设置窗口聚焦。
- 输入：事件唤醒、指针快照、Escape、指针采样节流。
- 测试探针（`test-hooks`）：轮询 / 事件计数、`popup_transitions_disabled` 等。

新增平台能力 = trait 加带默认实现的方法 → 对应外壳 override → 共享层只调用 trait。

## 菜单方案

- Windows：进程内专用 Win32 菜单线程 + `TrackPopupMenuEx(TPM_RETURNCMD | TPM_WORKAREA)`，
  命令回 eframe 线程；菜单打开时宠物继续动画。
- macOS：AppKit 原生菜单（含“选择宠物”checked 子菜单）。
- 不采用 WinUI3 / Windows App SDK：同进程 XAML Island 需要常驻 STA dispatcher 和 C++/WinRT shim，
  对四项菜单来说依赖、包体和首次弹出成本都高于收益。被否决方案的研究资料在 git 历史里
  （`docs/WINUI3_MENU.md`，2026-09-17 删除）。

## 发布轨道

```text
windows-v0.3.0  -> .github/workflows/release-windows.yml -> dist\Petsona-windows-<arch>-<version>.zip
macos-v0.2.1    -> .github/workflows/release-macos.yml   -> dist/Petsona.app + Petsona-macos-<arch>.zip
```

共享层变更至少保证两边编译 + 核心测试通过；触及平台能力时跑对应平台完整验收脚本。

## 分支策略

- 日常直接在 `main` 开发。
- 试验性改动用 `codex/win-*` / `codex/mac-*` 短期分支，合并后立即删除。
- 不长期维护 `windows` / `macos` 两套开发主线；如需稳定期可临时开 `release/win-*` 分支。

## 迁移状态

共享层（core / runtime / app 纯库）和平台外壳（Win32 / AppKit）都已就位；发布 workflow 已建立，
Windows 待首次 tag 执行，macOS 待实机复验与真实签名 / 公证。
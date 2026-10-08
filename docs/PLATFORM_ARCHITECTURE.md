# Petsona 平台架构（重建目标形态）

> 2026-10-08 起项目按 [desktop-shell-rust-ts](plans/desktop-shell-rust-ts.md) 计划重建。
> 旧 C#/WinUI 与 Swift/AppKit 前端及 C ABI 已从工作树删除，完整快照在 git `6bca241`；
> 本文件描述重建后的目标架构与边界，实现进度以执行记录为准。

## 分层

```text
crates/petsona-core + crates/petsona-runtime      （已存在，保留）
                 │  同进程 crate 调用（无 C ABI）
apps/desktop/src-tauri                            （M0 起建设）
  ├─ 平台层：托盘 / 自启 / 单实例 / 窗口几何 / 凭据 / 输入采样
  ├─ 浮层：宠物 / 气泡 / 编辑条 / Composer（原生窗口渲染）
  └─ 桥：runtime 命令与快照/事件 → Tauri commands / events
                 │  JSON 投影
apps/desktop/src                                  （TS 内容界面）
  └─ 设置六页 / 聊天 / 历史 / 人格来源 / 宠物库
```

## 职责边界

| 层 | 负责 |
|---|---|
| `petsona-core` | 宠物格式与图集、动画与注视规则、状态优先级、人格/记忆数据模型、兼容模型请求、协议服务器 |
| `petsona-runtime` | 配置与会话、宠物库、文件存储、实例锁、日志、问候、网络任务、流式聊天、记忆学习、人格生成与取消 |
| `src-tauri` 平台层 | 托盘与菜单、开机自启、单实例、显示器工作区与物理像素几何、输入/光标采样、凭据读写、窗口生命周期 |
| `src-tauri` 浮层 | 分层窗口逐像素渲染（宠物帧、气泡、编辑条、Composer）、穿透与命中、拖动/跟随 |
| `apps/desktop/src`（TS） | 内容页面与展示逻辑；通过 Tauri commands 读取投影、提交命令；不持有业务真相 |

## 窗口与线程模型

- **浮层窗口**：宠物 / 气泡 / 编辑条 / Composer 由 Rust 原生窗口承载并逐像素渲染；
  不激活、不抢焦点、可像素级命中/穿透；几何一律物理像素。
- **内容窗口**：设置 / 聊天 / 历史等为 Tauri WebView 窗口，可正常激活与键盘输入。
- **线程**：Tauri 主事件循环（内容窗口） + 浮层线程（自带消息泵，独立于 Tauri 窗口管理） +
  runtime 引擎 worker；命令入队、快照/事件出队，跨线程不传引用。
- 经验约束：拥有窗口的线程必须抽消息；跨线程同步消息会死锁（详见 `AGENTS.md` 坑与教训）。

## 几何与像素

- `PhysicalRect` 是唯一几何单位；逻辑点只在平台边界换算。
- 工作区夹取：Windows 用 `MonitorFromPoint` + `GetMonitorInfoW(rcWork)`（任务栏为边界）；
  macOS 用 `NSScreen.visibleFrame`（排除菜单栏与 Dock）。
- 混合 DPI、多屏、热插拔、自动隐藏任务栏属于新壳的验收范围（见验收清单 A5/F3）。

## 数据与协议（跨重建保持兼容）

- 数据目录：`%APPDATA%\Petsona` / `~/Library/Application Support/Petsona`，`PETSONA_HOME` 可覆盖。
- 配置：`config.json`；宠物库：本地 `pets/`；单实例锁：`petsona.lock`；日志：`logs/petsona.log`。
- 状态协议：`127.0.0.1:17872` 的 `POST /state`、`GET /health`、`GET /pets`；
  source 归属、优先级、TTL 与 `action:"clear"` 语义见 [DESKTOP_VERIFICATION](DESKTOP_VERIFICATION.md) 附录。
- 凭据只进系统凭据库（Windows Credential Manager / macOS Keychain），不进配置快照。

## 接口原则（Rust ↔ TS）

- 只传 JSON 投影与命令，不传 Rust 引用/容器/分配器所有权；不再有 C ABI。
- 快照字段沿用 `petsona-runtime` 现有投影（宠物状态、气泡计时、toast、几何、设置投影等），
  M1 起映射为 Tauri events；命令面沿用 runtime 的 command 语义（含 `ListModels`、气泡暂停等已实现行为）。
- 错误以结构化文本返回；长任务（模型拉取、人格生成、聊天流）通过事件推进度与结果。

## 发布

- 一个仓库、两端独立发布：`windows-v*` / `macos-v*` 各自 workflow（M6 重建）。
- 新壳首个正式版目标 `windows-v0.1.0`；Windows 打包需决定 WebView2 依赖策略（自包含或引导安装）。
- 自动检查与人工矩阵分开记录；编译通过不能替代窗口视觉、IME、托盘、登录自启与干净机器验收。

## 历史

- 第一代：Tauri + Preact（BytePet 时期），已删除。
- 第二代：Rust-only egui/eframe shell，已删除。
- 第三代：Windows C#/WinUI 3 + Win32、macOS SwiftUI/AppKit 原生前端（Windows 完成 W 矩阵、进入 rc.1 前），
  于 2026-10-08 整体删除——**当前为第四代（Rust + TypeScript 桌面壳）重建期**。
- 以上历史均可用只读 `git show` 查阅；不恢复死代码。

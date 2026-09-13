# BytePet 项目须知（给 AI 协作者）

## 这是什么

BytePet 是一个 Windows / macOS 桌宠：**Rust-only**（eframe/egui + winit + tray-icon + ureq），
没有 Node、WebView 或 Tauri 运行时。它读取 Codex 宠物包（`pet.json` + 8×9 / 8×11 图集），
按官方动画表播放，支持人格、轻量 JSON 记忆、DeepSeek 短问候，以及一个本地状态协议。

**平台现状（重要）**：Windows 是当前实测平台；macOS 交互后端已接入第一版，但尚未完成人工验收
（见「关键缺口」第 1 条和 `docs/MACOS_VERIFICATION.md`）。不要把 CI 的 macOS 绿灯当成
macOS 可用的证据。

## 目录

| 路径 | 内容 |
|---|---|
| `crates/bytepet-core/` | 宠物格式与动画引擎、人格、记忆、DeepSeek 客户端、本地状态协议 |
| `crates/bytepet-app/` | egui 应用；窗口/托盘/菜单/输入都在 `src/app.rs`，Win32 调用在 `src/platform.rs` |
| `legacy/` | 重构前的 Tauri 应用与前端，**仅作参考**，不参与构建 |
| `docs/PET_NATIVE.md` | Codex 原生宠物复刻的实测记录（帧表、行语义、验收步骤） |
| `docs/MACOS_VERIFICATION.md` | macOS 实机验收清单（基础回归 / 交互修复验收 / 多屏 / 打包） |

## 常用命令

```powershell
cargo run -p bytepet-app
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

两端完整验证：

```text
macOS:  bash scripts/verify-macos.sh
Windows: powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1
```

也可以在 Finder 双击 `scripts/verify-macos.command`，或在 Windows 资源管理器双击
`scripts\verify-windows.cmd`。脚本负责格式、Clippy、测试和 release 链接；窗口、托盘、菜单与
点击穿透仍需按 `docs/MACOS_VERIFICATION.md` 或 Windows 实机操作检查。

macOS 需要 Xcode Command Line Tools（`xcode-select --install`）和 Rust stable。

Windows MSVC 目标需要 VS Build Tools（“使用 C++ 的桌面开发”）。如果 shell 里没有 `link.exe`
（自动化沙箱里常见），可以退回 GNU 工具链：

```powershell
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = "$env:USERPROFILE\.rustup\toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\rust-lld.exe"
$env:RUSTFLAGS = "-C link-self-contained=yes"
cargo +stable-x86_64-pc-windows-gnu test --workspace
```

## 协作偏好（重要）

- **用中文交流。**
- **不要自动 `git add/commit/push`**，需要提交时用户会明确说。
- **`AGENTS.md` 由 AI 协作者直接维护**：内容可以自己看着改，不需要先征求同意；提交仍按上一条。
- **尽量不新增第三方依赖**：能用标准库 / 已有 `windows-sys` 解决的，不要引新 crate。
  macOS 全局指针状态（`NSEvent`）是少数值得破例的地方。
- 用户看不到你的屏幕、你也截不了图；Windows 上可以用 PowerShell + Win32 读窗口样式/位置（见下），
  macOS 没有等价的自省脚本。涉及窗口、托盘、菜单的改动，交付时写清“需要用户实机确认什么”。
- 改动较大时先说清思路再动手；改完至少跑 `fmt` / `clippy` / `test`。

## 当前状态（2026-09-13）

- 最近一次全量测试 60 个通过（core 54 + app 6）；macOS A3、B1–B7 已由用户实测通过，B10 暂无副屏条件。
- CI：`pull_request` → main、`push` tag `v*`、`workflow_dispatch`；直接推 `main` 不跑；
  同一 ref 的旧运行会被 concurrency 取消。
- 已完成（**以 Windows 实测为准**）：宠物格式与动画（含 V2 look 行“注视”）、宠物库
  （本地库 + 只读引用 `~/.codex/pets`、`~/.unipet/pets`，支持导入文件夹/zip、导出、删除，
  内置 ByteBot 常驻本地库）、人格 + JSON 记忆 + DeepSeek 问候、托盘（Windows 使用自绘菜单，
  macOS 使用原生菜单）、设置窗口（可滚动、原生边框、用宠物首帧当图标）、
  本地状态协议、真·无边框宠物窗口、点击/拖动/双击/右键/看向光标/活动提醒行走、单实例锁、
  文件日志、按需重绘，以及 macOS `.app`/LaunchAgent/签名/公证脚本。

## 关键缺口（按优先级）

### 1. macOS 交互后端（核心交互已通过；当前剩余是多屏与功耗实测）

`crates/bytepet-app/src/platform.rs` 已接入 macOS 原生后端：

- `NSEvent::mouseLocation`：全局光标位置，并转换为 winit 的屏幕坐标；
- `NSEvent::pressedMouseButtons`：主/次按键状态；
- CoreGraphics `CGEventSourceKeyState`：Escape 状态；
- AppKit 非激活窗口样式：宠物和菜单不抢前台焦点；
- `winit::Window::set_cursor_hittest`：继续由现有 `MousePassthrough` 路径切换点击穿透。

代码已经通过编译、clippy 和自动化测试；用户已确认 A3、B1–B7。仍需按
`docs/MACOS_VERIFICATION.md` 实测 B10 多显示器坐标、B11 空闲 CPU，以及 C 节的 Retina/Spaces 行为。

平台差异仍保持如下：

- Windows：继续用现有 Win32（`GetCursorPos` / `GetAsyncKeyState` / `WS_EX_NOACTIVATE`）。
- macOS：使用 `NSEvent` 和 AppKit；只靠 egui 事件拿不到窗口外的全局光标。
  `winit` 0.30 的 `Window::set_cursor_hittest` 在 macOS 可用，缺的是全局坐标/按键，不是穿透 API。
- `NSEvent::mouseLocation` 是屏幕坐标，原点/Y 方向与 winit 的坐标约定不同；多显示器下要通过
  winit 的 monitor 几何做映射，别直接混用。

验收：按 `docs/MACOS_VERIFICATION.md` 的 B 节逐条过。

### 2. 日常可用性（发布形态）

基础发布能力已经补齐，仍需目标机器做最终验收：

- **单实例**：`crates/bytepet-app/src/instance_lock.rs` 使用数据目录锁文件；同一
  `BYTEPET_HOME` 下第二个实例会退出，锁随进程结束自动释放。
- **日志**：`logs/bytepet.log` 与终端双写；release 已隐藏 Windows 控制台，Finder/LaunchAgent
  启动失败时仍可查看文件日志。
- **macOS 打包**：`scripts/package-macos.sh` 生成 `.app` 与架构包 zip，含 Info.plist、bundle id、
  图标和 `LSUIElement`。
- **macOS 开机自启**：`scripts/install-macos-launch-agent.sh` 安装/卸载 LaunchAgent；需要在目标
  用户会话中实际执行并验收。
- **签名/公证**：`scripts/sign-macos.sh` 和 `scripts/notarize-macos.sh` 已提供流程，但实际执行
  需要用户的 Developer ID 证书和 `notarytool` profile。
- **发布工作流**：`.github/workflows/release-macos.yml` 在 tag 或手动触发时生成架构包；默认产物
  未签名，签名/公证需在有凭据的环境中执行。

### 3. 重绘预算（省电）

- `BytePetApp::schedule_repaint` 按动画帧时长、气泡、点击判定、拖拽/自动行走和交互轮询安排
  下一次重绘；idle 不再无条件按 60 FPS 重绘。
- 状态协议和后台问候完成时会主动 wake event loop；全局鼠标在需要像素穿透/转头时保留低频兜底轮询。
- 仍需在 Activity Monitor 实测 B11，确认目标机器空闲 CPU 接近 0–1%。

> 原先“偶发闪 / 气泡独立窗口 / 菜单物理像素定位 / 记住位置 / 重力开关”那批 Windows 待办
> 已从本文件移除；需要时从 git 历史或对话里找回。

## 已踩过的坑（别再重新推导）

除特别标注外都是 Windows 结论：

- **winit 的 `decorations(false)` 不是真的无边框**：窗口仍带 `WS_CAPTION | WS_BORDER | WS_DLGFRAME |
  WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX`，只靠 `WM_NCCALCSIZE` 装样子。任何状态变化
  （激活、切样式、改尺寸）都会让 Windows 重画那个边框——这就是用户反复报告的“闪现”。
  做法：`SetWindowLongPtrW(GWL_STYLE)` 去掉这些位、加上 `WS_POPUP`，再 `SetWindowPos(SWP_FRAMECHANGED)`；
  随后必须重新 `DwmEnableBlurBehindWindow`（空 blur 区域）恢复透明，因为 frame 重算会把 winit 设的透明弄丢。
  实测：`style=0x96000000 POPUP=True CAPTION=False`。
- **不要每帧重复发 `ViewportCommand::InnerSize` / `WindowLevel`**：会反复 `SetWindowPos` → 闪。
  只在该变的时候发（`app.rs` 里有 `applied_window_size` / `applied_always_on_top` 缓存）。
- **`tray-icon` 的原生菜单会卡死 Windows 程序**：Windows 的 `TrackPopupMenu` 在事件循环线程上开模态循环，
  期间宠物不重绘、“退出”也发不出去，所以 Windows 使用 `TrayIconEvent::Click` 弹自己的窗口；
  macOS 使用 AppKit 原生菜单。
- **不激活窗口很有用**：宠物 / 气泡 / 菜单都不该抢焦点（点宠物不该打断用户正在编辑的窗口）。
  Windows 使用 `WS_EX_NOACTIVATE`，macOS 使用 AppKit 的非激活面板样式，均需实机确认。
- **状态协议曾经的“偶发空响应”**：Windows 上 `accept()` 得到的 socket 会继承监听 socket 的非阻塞模式，
  读请求时 `WouldBlock` 就把连接丢掉。修法：accept 后显式 `set_nonblocking(false)`（见 `state_server.rs`
  的回归测试 `every_request_gets_a_response`）。
- **HTTRANSPARENT 的坑**：`WM_NCHITTEST` 返回 `HTTRANSPARENT` 只在**同线程**窗口间可靠转发鼠标，
  跨进程要靠 `WS_EX_TRANSPARENT`（但那个会引入上面的抖动）。也就是“像素级穿透”和“零闪烁”要权衡。
- **macOS 只靠 egui 事件拿不到窗口外的全局光标**：egui 只看到投递给本窗口的事件；
  转头、拖拽、点击穿透都需要全局光标/按键状态，必须走 macOS 原生 API（或先做窗口内降级）。

## 关键事实速查

- 数据目录：`%APPDATA%\BytePet`（mac：`~/Library/Application Support/BytePet`），可用环境变量
  `BYTEPET_HOME` 覆盖（测试 / 便携用）。本地宠物库在 `...\BytePet\pets`，另有只读引用
  `~/.codex/pets`、`~/.unipet/pets`；同 id 时本地库优先。
- 状态协议：`127.0.0.1:17872`，`POST /state`（`{source,state,message,ttlMs}`）、`GET /health`、
  `GET /pets`；`ttlMs: 0` 表示不过期。状态名见 `docs/PET_NATIVE.md`。
- 发行版宠物实测：V2 = 8×11；**idle 画了 7 帧**（官方时长表写 6），引擎按“真正画了内容的格子”取帧数；
  第 9 / 10 行是“转过去再转回来”的一次性动作，实测**第 9 行 = 转向右，第 10 行 = 转向左**
  （用头部深色像素重心相对头部中心测得，先拿 row1 / row2 校准过）。详见 `docs/PET_NATIVE.md`。
- 窗口样式自查（**Windows only**，比截图可靠）：用 `EnumWindows` 找本进程里标题为 `BytePet` /
  `BytePet 气泡` / `BytePet 菜单` 的窗口，再用 `GetWindowLongPtrW(hwnd, GWL_STYLE / GWL_EXSTYLE)`
  看 `POPUP` / `CAPTION` / `TRANSPARENT` / `NOACTIVATE` 位。macOS 没有等价脚本，靠
  `docs/MACOS_VERIFICATION.md` 人工验收。
- release profile：`lto = "thin"`、`codegen-units = 1`、`strip = true`、`panic = "abort"`；日志写入
  数据目录的 `logs/bytepet.log`，崩溃前的 tracing 通常会留下线索。

## Git 工作流

个人仓库、Windows + macOS 两地开发，直接在 `main` 上做：

```powershell
git fetch --prune
git switch main
git pull --ff-only
# ...改代码...
git add -A
git commit -m "..."
git push
```

全局配置（`pull.ff=only`、`push.autoSetupRemote=true`、`fetch.prune=true`、`core.longpaths=true`）已设好。

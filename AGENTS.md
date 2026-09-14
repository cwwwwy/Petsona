# Petsona 项目须知（给 AI 协作者）

## 这是什么

Petsona 是一个 Windows / macOS 桌宠：**Rust-only**（eframe/egui + winit + tray-icon + ureq），
没有 Node、WebView 或 Tauri 运行时。它读取 Codex 宠物包（`pet.json` + 8×9 / 8×11 图集），
按官方动画表播放，支持人格、轻量 JSON 记忆、DeepSeek 短问候，以及一个本地状态协议。

**平台现状（重要）**：Windows 是当前实测平台；macOS 交互后端已接入第一版，但尚未完成人工验收
（见「关键缺口」第 1 条和 `docs/MACOS_VERIFICATION.md`）。不要把 CI 的 macOS 绿灯当成
macOS 可用的证据。

## 目录

| 路径 | 内容 |
|---|---|
| `crates/petsona-core/` | 宠物格式与动画引擎、人格、记忆、DeepSeek 客户端、本地状态协议 |
| `crates/petsona-app/` | egui 应用；窗口/托盘/菜单/输入都在 `src/app.rs`，Win32 调用在 `src/platform.rs` |
| `legacy/` | 重构前的 Tauri 应用与前端，**仅作参考**，不参与构建 |
| `docs/PET_NATIVE.md` | Codex 原生宠物复刻的实测记录（帧表、行语义、验收步骤） |
| `docs/MACOS_VERIFICATION.md` | macOS 实机验收清单（基础回归 / 交互修复验收 / 多屏 / 打包） |
| `docs/WINDOWS_VERIFICATION.md` | Windows 实机验收清单（基础回归 / Win32 交互 / 多屏 / 打包） |
| `docs/WINUI3_MENU.md` | 同进程 WinUI3 XAML Island 菜单的准备条件、架构和性能策略 |

## 常用命令

```powershell
cargo run -p petsona-app
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

两端完整验证：

```text
macOS:  bash scripts/verify-macos.sh
Windows 快速门禁: powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1
Windows 完整门禁: powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1 -Full
Windows smoke: powershell -ExecutionPolicy Bypass -File scripts\windows-smoke.ps1
```

也可以在 Finder 双击 `scripts/verify-macos.command`，或在 Windows 资源管理器双击
`scripts\verify-windows.cmd`。脚本负责格式、Clippy、测试和 release 链接；窗口、托盘、菜单与
点击穿透仍需按 `docs/MACOS_VERIFICATION.md` 或 `docs/WINDOWS_VERIFICATION.md` 检查。

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
- **本文件是跨会话/跨机器的上下文载体**：新会话先读 AGENTS.md、docs/PET_NATIVE.md、docs/MACOS_VERIFICATION.md、docs/WINDOWS_VERIFICATION.md、docs/WINUI3_MENU.md 和 git log；重要决策和计划要及时写回本文件。
- **尽量不新增第三方依赖**：能用标准库 / 已有 `windows-sys` 解决的，不要引新 crate。
  macOS 全局指针状态（`NSEvent`）是少数值得破例的地方。
- 用户看不到你的屏幕、你也截不了图；Windows 上可以用 PowerShell + Win32 读窗口样式/位置（见下），
  macOS 没有等价的自省脚本。涉及窗口、托盘、菜单的改动，交付时写清“需要用户实机确认什么”。
- 改动较大时先说清思路再动手；改完至少跑 `fmt` / `clippy` / `test`。

## 当前状态（2026-09-14）

- 项目已从 BytePet 改名为 **Petsona**；本机仓库目录为 `C:\Users\happyddz\Desktop\Petsona`
  （原 `Desktop\bytepet` 已不存在）。GitHub remote 仍是 `https://github.com/cwwwwy/bytepet.git`。
- 当前分支 `codex/cross`（已推送 origin；验证清单更新尚未提交），比 `main` 多 3 个提交：
  `8398a0d` macOS native menus、`7f48c4b` desktop usability + macOS release flow、
  `20cd2ba` Rename BytePet to Petsona。`main` 在 `af99831`（跨平台 verify 脚本）。
- 最近一次全量测试 60 个通过（core 54 + app 6）。
- 平台验收：macOS A3、B1–B7 已实测通过；B8/B9/B11/B12/B13、A1/A2/A4–A11、C 节、真实签名/公证
  待实测；B10 因没有副屏暂缓。
- 菜单决策（2026-09-14）：不引入 WinUI3/Windows App Runtime；采用同进程 Win32 系统原生菜单线程，优先保证性能、主题一致性和零额外运行时。缩放和 B11 继续并行修正。
- Windows 自动门禁：`scripts\verify-windows.ps1` 已于 2026-09-14 通过（fmt / clippy / test / release）；
  `-Full` 已接入 smoke 和 `test-hooks`，自动通过 15 项：T1、A1、A4（设置，部分）、A8、A9、A11（部分）、A13（加速，部分）、B7（Win32 激活守卫）、B8（样式/缩放稳定）、B9（几何，部分）、B11（鼠标事件唤醒）、B12–B14、C6。
- Windows 实机状态（用户 2026-09-14 更新，详见 `docs/WINDOWS_VERIFICATION.md`）：
  A1–A7、A10–A12 通过，A8/A9 的协议层已由 smoke 自动通过，A13 属长测；B1–B6、B9、B10、B12–B14 通过，
  B5 需由“一次性转头”增强为持续注视；B7 已加入 `WM_MOUSEACTIVATE -> MA_NOACTIVATE` 守卫，B8 已加入 `WM_STYLECHANGING` 守卫，并修复缩放底部中心锚点与命中掩码缩放；B11 已改为低层鼠标事件唤醒并缓存坐标，真实桌面 CPU/GPU 仍需实机确认；B7/B8 的肉眼/真实输入仍需实机确认；
  C1 通过，C2/C3 待 Phase 2，C4–C5 待实测，C6 的宠物窗口与菜单样式已自动化通过。
- 分支策略（2026-09-14）：用户暂不提交 PR、不合并 `main`，继续在 `codex/cross` 上开发；
  合并时机由用户决定。
- 改名决策（2026-09-14）：**不提供 BytePet → Petsona 数据迁移**，开发验证阶段接受从零开始，
  不读取旧 config/personas/memory/pets。
- CI：`pull_request` → main、`push` tag `v*`、`workflow_dispatch`；直接推 `main` 不跑；
  同一 ref 的旧运行会被 concurrency 取消。

## Windows 开发计划（2026-09-14，修订 2）

当前实机状态以 `docs/WINDOWS_VERIFICATION.md` 为准。下一步先在 `codex/cross` 上做自动化骨架
和基线修复，不提交 PR、不合并 `main`；合并时机由用户决定。

### Phase 0.5 — 自动化验收骨架（第一版已完成）

目标是让日常改动不再逐条手点：

- `scripts\verify-windows.ps1` 快速门禁和 `-Full` 完整门禁均已实现：fmt / clippy / test /
  release build 不启动 GUI；`-Full` 额外调用 `scripts\windows-smoke.ps1`。
- `scripts\windows-smoke.ps1` 已实现，用临时 `PETSONA_HOME` 启动 release，当前自动通过 12 项：
  T1、A1、A4（设置，部分）、A8、A9、A11（部分）、A13（加速，部分）、B9（几何，部分）、
  B12–B14、C6（宠物窗口与菜单样式）。
- `test-hooks` 已实现并默认关闭：本机控制端口提供窗口/气泡/注视/重绘/指针轮询/样式重应用快照，
  以及打开菜单和设置、隐藏/显示、气泡、缩放、穿透、自动行走、保存、退出等确定性动作。
- 自动行走已改为浮点逻辑坐标累加，避免步长小于 1px 时停住。
- 自动化分三层：快速门禁每次改动跑；`-Full` 在窗口/输入/配置变更后跑；视觉、真实托盘点击、
  多屏、DPI、全屏/虚拟桌面和发布环境只做最终人工验收。
- 不新增第三方依赖，Win32 交互继续使用现有 `windows-sys`，脚本用 PowerShell。

预计自动化覆盖 A1/A4 内部动作/A8–A11/A13、B1–B4/B6 样式/B7/B9–B14、C6、D1/D3/D4；
托盘图标外观与真实点击、B6 实际桌面落点、B8 肉眼闪烁、C1–C5、D2/D5/D6 仍保留人工。

### Phase 0.6 — Windows 基线缺陷（修复顺序固定）

1. **B7 不抢焦点（代码与自动守卫已完成）**：宠物和菜单已子类化窗口过程，对
   `WM_MOUSEACTIVATE` 返回 `MA_NOACTIVATE`；smoke 直接验证返回值，并要求鼠标进入宠物前后
   前台窗口不变。当前会话禁止 SendInput，真实点击仍需实机确认。
2. **B8 无边框闪（代码与自动守卫已完成）**：确认 winit 在尺寸/标志变化时会重设
   `GWL_STYLE/GWL_EXSTYLE`。现在通过 `WM_STYLECHANGING` 在应用前恢复 `WS_POPUP` 并保留
   `WS_EX_NOACTIVATE`；smoke 已验证缩放、菜单、设置和穿透切换不再触发 frame-style 重建。
   缩放时窗口会延后补偿外框位置，保持宠物底部中心锚点；命中掩码坐标按 `window.scale`
   还原到未缩放图集空间，单元测试覆盖 0.75/1.0/1.5 三档。肉眼闪烁仍需最终确认。
3. **B11 空闲功耗（代码已完成，实机功耗待确认）**：Windows 已改用 `WH_MOUSE_LL` 低层鼠标事件
   唤醒，并缓存最近光标位置；空闲时不再用 100ms 全局 `GetCursorPos` 轮询。smoke 在交互桌面
   可用时执行 CPU/轮询门槛，无交互桌面时明确跳过。下一步是实机 Activity Monitor/任务管理器确认。
4. **B5 持续注视**：把 row9/row10 从一次性 glance 改为可保持的注视状态。光标进入左右触发区后
   停在最强方向帧，未离开触发区时保持；离开或回到死区后播放返回段并回 base。先补纯状态机单测，
   再用 smoke 移动全局光标验证。

### Phase 1 — 窗口 + 气泡

气泡独立成透明、点击穿透、`with_active(false)`、无边框、置顶的窗口，固定在宠物上方并跟随移动，
无气泡时完全隐藏；宠物窗口去掉 `BUBBLE_AREA_HEIGHT` 收缩到精灵精确尺寸；保持 `WS_POPUP` +
DWM 透明 + `WS_EX_NOACTIVATE`，不在像素级切换穿透路径中重新引入 frame 重算。

### Phase 2 — 多屏 + 位置记忆

菜单用 `GetCursorPos` + `MonitorFromPoint` + `GetMonitorInfoW(rcWork)` + `SetWindowPos`
物理像素落位；拖动结束把物理坐标写入 `window.startPosition`；启动精确还原，保存的显示器不存在时
回落；首次/重置位置放主屏工作区右下角（约 32px）。C2/C3 完成后必须由 smoke 自动检查坐标。

### Phase 3 — 重力开关

设置 → 宠物行为，默认关；约 2600 px/s²、上限约 1800 px/s；落到当前显示器工作区底部并播放
一次 `jumping`；拖动或自动行走时不生效。

### Phase 4 — Windows 发布形态

`packaging/windows/Petsona.ico` 并嵌入 exe；`scripts/package-windows.ps1` 出 zip；
`release-windows.yml`；HKCU Run/Startup 自启开关；维护 `docs/WINDOWS_VERIFICATION.md`。

### 菜单方案决策：同进程 Win32 系统原生菜单（2026-09-14）

用户目标是兼顾性能与美观，不强制 WinUI3。WinUI3 同进程 XAML Islands 需要 Windows App
Runtime、常驻 XAML dispatcher、DesktopWindowXamlSource 和同进程 C++/WinRT shim，对四项菜单
来说复杂度和运行时代价过高，因此不作为当前实现方案；研究资料保留在 `docs/WINUI3_MENU.md`。

采用方案：

- 在 Petsona 进程内创建专用 Win32 菜单线程和隐藏 owner window。
- 使用 `CreatePopupMenu` / `AppendMenuW`。
- 使用 `TrackPopupMenuEx(TPM_RETURNCMD | TPM_NONOTIFY | TPM_WORKAREA)`。
- 菜单命令 id 通过 channel/PostMessage 返回 eframe 主线程。
- `TrackPopupMenuEx` 不运行在 eframe 事件循环线程，因此不会冻结宠物动画。
- 使用系统原生主题、键盘导航、暗色模式、DPI 和显示器工作区行为。

性能边界：

- 空闲时菜单线程只等待消息，不参与重绘和输入轮询。
- 菜单对象一次创建并复用；关闭后只隐藏，不销毁线程。
- 不引入 Windows App SDK、.NET、CLR、XAML 或额外运行时。
- 继续使用现有 `windows-sys`，不新增 crate。

建议分期：

1. 先实现托盘菜单；它最适合系统原生外观，也不涉及宠物像素命中。
2. 宠物右键菜单暂留现有 egui 自绘版本，避免影响 B7 焦点和点击穿透。
3. 托盘菜单稳定后，再用同一菜单线程替换宠物右键菜单，并跑焦点/多屏/DPI 回归。
4. WinUI3 仅作为未来独立 spike，不进入主构建，除非 Win32 原生菜单的视觉最终不能接受。
### Phase 5 — 发布质量

Fast + Full 验证全绿；手工清单只剩视觉/真实桌面/多屏/DPI/登录自启/Defender；菜单工作区夹取
和位置回退写成纯函数并加单测；Windows 不重写现有 Win32 后端，只做增量。
## 关键缺口（按优先级）

### 1. macOS 交互后端（核心交互已通过；当前剩余是多屏与功耗实测）

`crates/petsona-app/src/platform.rs` 已接入 macOS 原生后端：

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

- **单实例**：`crates/petsona-app/src/instance_lock.rs` 使用数据目录锁文件；同一
  `PETSONA_HOME` 下第二个实例会退出，锁随进程结束自动释放。
- **日志**：`logs/petsona.log` 与终端双写；release 已隐藏 Windows 控制台，Finder/LaunchAgent
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

- `PetsonaApp::schedule_repaint` 按动画帧时长、气泡、点击判定、拖拽/自动行走和交互轮询安排
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

- 数据目录：`%APPDATA%\Petsona`（mac：`~/Library/Application Support/Petsona`），可用环境变量
  `PETSONA_HOME` 覆盖（测试 / 便携用）。本地宠物库在 `...\Petsona\pets`，另有只读引用
  `~/.codex/pets`、`~/.unipet/pets`；同 id 时本地库优先。
- 状态协议：`127.0.0.1:17872`，`POST /state`（`{source,state,message,ttlMs}`）、`GET /health`、
  `GET /pets`；`ttlMs: 0` 表示不过期。状态名见 `docs/PET_NATIVE.md`。
- 发行版宠物实测：V2 = 8×11；**idle 画了 7 帧**（官方时长表写 6），引擎按“真正画了内容的格子”取帧数；
  第 9 / 10 行是“转过去再转回来”的一次性动作，实测**第 9 行 = 转向右，第 10 行 = 转向左**
  （用头部深色像素重心相对头部中心测得，先拿 row1 / row2 校准过）。详见 `docs/PET_NATIVE.md`。
- 窗口样式自查（**Windows only**，比截图可靠）：用 `EnumWindows` 找本进程里标题为 `Petsona` /
  `Petsona 气泡` / `Petsona 菜单` 的窗口，再用 `GetWindowLongPtrW(hwnd, GWL_STYLE / GWL_EXSTYLE)`
  看 `POPUP` / `CAPTION` / `TRANSPARENT` / `NOACTIVATE` 位。macOS 没有等价脚本，靠
  `docs/MACOS_VERIFICATION.md` 人工验收。
- release profile：`lto = "thin"`、`codegen-units = 1`、`strip = true`、`panic = "abort"`；日志写入
  数据目录的 `logs/petsona.log`，崩溃前的 tracing 通常会留下线索。

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

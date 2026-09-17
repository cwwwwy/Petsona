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
| `crates/petsona-runtime/` | 平台无关运行时：配置/人格/记忆/宠物库、状态协议、实例锁、日志、问候 |
| `crates/petsona-app/` | 共享 egui UI 与 `platform::PlatformHost` 边界（库，无平台代码，无二进制） |
| `crates/petsona-shell-windows/` | Windows 外壳：Win32 窗口样式、`WH_MOUSE_LL` 钩子、弹菜单线程（bin `petsona-windows`） |
| `crates/petsona-shell-macos/` | macOS 外壳：AppKit/NSEvent/CoreGraphics 后端、系统托盘菜单（bin `petsona-macos`） |
| `legacy/` | 重构前的 Tauri 应用与前端，**仅作参考**，不参与构建 |
| `docs/PET_NATIVE.md` | Codex 原生宠物复刻的实测记录（帧表、行语义、验收步骤） |
| `docs/MACOS_VERIFICATION.md` | macOS 实机验收清单（基础回归 / 交互修复验收 / 多屏 / 打包） |
| `docs/WINDOWS_VERIFICATION.md` | Windows 实机验收清单（基础回归 / Win32 交互 / 多屏 / 打包） |
| `docs/WINUI3_MENU.md` | 同进程 WinUI3 XAML Island 菜单的准备条件、架构和性能策略 |

## 常用命令

```powershell
cargo run -p petsona-shell-windows   # Windows 外壳（真正的产品入口）
cargo run -p petsona-shell-macos     # macOS 外壳
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

## 脚本约定

- 每个平台只保留一个验收入口；不要新增 `.cmd` / `.command` 包装。
- 长流程 smoke 可独立成文件，供入口调用，也便于对已构建产物单独复跑。
- 打包脚本各保留一份；资源生成脚本放在对应资源目录，不堆进 `scripts/`。
- 新增脚本前先检查能否并入现有入口或实现文件。

## 两端验收入口

每边只需要记住一个脚本，不再有 `.cmd` / `.command` 双击包装：

```text
macOS   完整: bash scripts/verify-macos-all.sh
        Rust 门禁: bash scripts/verify-macos-all.sh --gates-only
Windows 完整: powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1 -Full
        快速: powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1
```

入口脚本负责格式、Clippy、测试、release 链接、自动 smoke 和打包结构检查。
`scripts\windows-smoke.ps1`、`scripts\macos-smoke.sh` 是 smoke 实现文件（较长且可对已构建产物单独重跑），
只在定位失败时单独运行；
窗口、托盘、菜单与点击穿透的肉眼/真实输入部分仍需按 `docs/MACOS_VERIFICATION.md` 或
`docs/WINDOWS_VERIFICATION.md` 检查。

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
- **Git 相关操作一律由用户手动执行（2026-09-16 起，用户明确要求）**：AI 只做**只读查询**
  （`git status` / `log` / `diff` / `remote -v` / `branch -vv`）来判断状态，然后**给出命令让用户自己跑**。
  不要代为 `add` / `commit` / `push` / 建删分支 / 改 remote / 打 tag / 改仓库设置（`gh repo edit` 等）；
  只有用户明确说"你来操作"时才例外。
- **仓库对外元数据要和 Petsona 同步**：改名、改定位、加功能后，除代码外还要检查 GitHub 的仓库名、
  About 描述、主页、Topics（以及 README 第一段口径）。当前建议值见「GitHub 仓库元数据」一节。
- **`AGENTS.md` 由 AI 协作者直接维护**：内容可以自己看着改，不需要先征求同意；提交仍按上一条。
- **本文件是跨会话/跨机器的上下文载体**：新会话先读 AGENTS.md、docs/PET_NATIVE.md、docs/MACOS_VERIFICATION.md、docs/WINDOWS_VERIFICATION.md、docs/WINUI3_MENU.md 和 git log；重要决策和计划要及时写回本文件。
- **尽量不新增第三方依赖**：能用标准库 / 已有 `windows-sys` 解决的，不要引新 crate。
  macOS 全局指针状态（`NSEvent`）是少数值得破例的地方。
- 用户看不到你的屏幕、你也截不了图；Windows 上可以用 PowerShell + Win32 读窗口样式/位置（见下），
  macOS 没有等价的自省脚本。涉及窗口、托盘、菜单的改动，交付时写清“需要用户实机确认什么”。
- 改动较大时先说清思路再动手；改完至少跑 `fmt` / `clippy` / `test`。

## 当前状态（2026-09-17）

- 项目已从 BytePet 改名为 **Petsona**；规范仓库地址为 `https://github.com/cwwwwy/Petsona.git`。
  旧 `bytepet` 地址由 GitHub 重定向；本机 `origin` 若仍是旧地址，应由用户手动更新。
- 当前共享开发分支为 `main`；不要在本文固定记录 commit hash，确切版本用 `git log -1 --oneline` 查询。
- 平台架构为共享 `petsona-core` / `petsona-runtime` 加独立平台 shell；完整说明见 `docs/PLATFORM_ARCHITECTURE.md`。
- 历史自动化结果不保证覆盖当前提交；代码改动后须运行对应平台验收入口，人工项目另按验收清单检查。
- 平台实测状态见两端验收清单；不要仅凭 smoke 或历史记录推断窗口、菜单、注视动画和功耗已经通过。
- 菜单决策：macOS 使用 AppKit 原生菜单，Windows 使用专用 Win32 菜单线程；不引入 WinUI3 / Windows App Runtime。方案背景见 `docs/WINUI3_MENU.md`。
- Windows 自动化覆盖与逐项人工状态统一记在 [`docs/WINDOWS_VERIFICATION.md`](docs/WINDOWS_VERIFICATION.md)。
- 用户暂时没有 Windows 实体机；需真实桌面、输入、多屏或发布环境的条目先保持待验收，不以自动化代签。
- macOS 自动化覆盖与逐项人工状态统一记在 [`docs/MACOS_VERIFICATION.md`](docs/MACOS_VERIFICATION.md)。
- 分支策略：日常直接在 `main` 开发；如需试验，使用短期 `codex/*` 分支，合并与清理由用户决定。
- 改名决策（2026-09-14）：**不提供 BytePet → Petsona 数据迁移**，开发验证阶段接受从零开始，
  不读取旧 config/personas/memory/pets。
- CI：`pull_request` → main、`push` tag `v*`、`workflow_dispatch`；直接推 `main` 不跑；
  同一 ref 的旧运行会被 concurrency 取消。

## 平台架构决策

用户确认两端可独立规划和发布（Windows 约 70% / macOS 约 30%）：一个仓库、一个 workspace、共享
`petsona-core` / `petsona-runtime`，平台窗口、菜单、输入和打包分别放在 Windows / macOS shell。
`petsona-app` 通过 `PlatformHost` 接收平台能力，不新增平台 `#[cfg(target_os)]`。

- 日常共享主线是 `main`；确有需要时只开短期 `codex/*` 分支。
- 发布轨道：`windows-v*` 与 `macos-v*`；CI / release workflows 已建立。
- Windows 菜单使用 Win32 专用菜单线程，macOS 使用 AppKit；不引入 WinUI3 / Windows App Runtime。
- 新增平台能力优先给 `PlatformHost` 提供可移植默认实现，再由外壳 override。

架构细节和迁移状态见 [`docs/PLATFORM_ARCHITECTURE.md`](docs/PLATFORM_ARCHITECTURE.md)。

## GitHub 仓库元数据（2026-09-17）

- 规范仓库地址：`https://github.com/cwwwwy/Petsona.git`（旧 `bytepet` 地址由 GitHub 自动重定向；
  如本机 `origin` 仍指向旧地址，由用户手动执行 `git remote set-url origin https://github.com/cwwwwy/Petsona.git` 更新）。
- About 描述（建议，英文，GitHub 上限 350 字符）：
  `Rust-only desktop pet for Windows & macOS — Codex pet packs, native menus, persona + light memory. No Node/WebView.`
- Topics（建议）：`rust` `desktop-pet` `egui` `eframe` `windows` `macos` `codex` `tray-icon`
- 主页：暂时留空，有官网/演示页再填。
- 修改方式（两种都行）：GitHub 网页仓库首页 About 区域 → 齿轮 → 填描述/主页/Topics → Save；
  或命令行 `gh repo edit --description "..." --add-topic rust --add-topic desktop-pet ...`。
- 已核对与 Petsona 口径一致、无需改动的地方：README 标题与首段、各 crate 包名（`petsona-*`）、
  数据目录（`%APPDATA%\Petsona` / `~/Library/Application Support/Petsona`）、macOS bundle id
  （`com.petsona.desktop`）。`legacy/` 里保留旧 BytePet 代码作为参考，不做改名。

## Windows 实施与验收

功能实现与自动化状态见 `docs/WINDOWS_VERIFICATION.md`。验收分为可脚本重复的门禁 / smoke
和必须在真实桌面观察的窗口、输入、DPI、多屏与发布行为；不要把历史计划当作当前待办。

### 自动化入口

Windows 快速门禁与完整 smoke 使用同一脚本：`scripts\verify-windows.ps1`，完整模式增加 `-Full`。
覆盖范围和未自动化的人工验收以 `docs/WINDOWS_VERIFICATION.md` 为准；这里不再复制检查项清单。

Windows 窗口、焦点、缩放、功耗、注视、位置记忆和重力的实现 / 实测细节均由
`docs/WINDOWS_VERIFICATION.md` 维护；平台特有的根因与修复经验集中在下方「已踩过的坑」。

Windows 打包、图标、自启和 release workflow 已实现；仍需目标机器确认的项目见
`docs/WINDOWS_VERIFICATION.md`。原生菜单方案与 WinUI3 取舍见 `docs/WINUI3_MENU.md`。

## 阶段 7 已实现功能（2026-09-16）

以下功能已落地；逐项自动化与人工验收状态以两端验收清单为准，不在这里重复维护。

- Superintendent 内置宠物与 Codex 宠物显式导入已完成；实现细节见 `docs/PET_NATIVE.md` 与对应代码测试。

- Windows 开机自启、物理像素位置保存 / 屏幕外回落、菜单工作区夹取和可选重力已实现；
  自动化 / 人工状态见 `docs/WINDOWS_VERIFICATION.md`。
- 人工验收只在 `docs/WINDOWS_VERIFICATION.md` 的 A–D 表中维护，避免另建状态清单。
- 首次正式 Windows 发布仍待执行；macOS Developer ID 签名 / 公证需要相应凭据。

- macOS 侧先运行 `bash scripts/verify-macos-all.sh`；注视、气泡与对话视觉、缩放闪动和 Activity Monitor 仍需确认。
  多显示器 / Retina 按用户决定暂缓；验收状态只维护在 `docs/MACOS_VERIFICATION.md`。

## 关键缺口（按优先级）

### 1. macOS 实机验收（状态以专用清单为准）

`crates/petsona-shell-macos/src/platform.rs` 是 macOS 原生后端（2026-09-16 从 `petsona-app` 搬出，
逻辑未改）：

- `NSEvent::mouseLocation`：全局光标位置，并转换为 winit 的屏幕坐标；
- `NSEvent::pressedMouseButtons`：主/次按键状态；
- CoreGraphics `CGEventSourceKeyState`：Escape 状态；
- `PointerSnapshot`：macOS 空闲时最多每 100ms 采样一次光标和鼠标按键，按钮/移动事件、拖动和菜单期间立即刷新；坐标 Y 翻转使用 CoreGraphics 主屏高度；
- 右键/左键按下同时消费 egui 事件边沿和全局状态轮询，避免短促触控板按键落在轮询间隔内；命中测试优先使用事件的窗口内坐标；
- macOS 原生“选择宠物”子菜单使用 `CheckMenuItem` 标记当前宠物；设置窗口创建后发送 `Focus`，并由 AppKit `makeKeyAndOrderFront` + `NSApplication::activate` 兜底；
- AppKit 非激活窗口样式：宠物和菜单不抢前台焦点；
- `winit::Window::set_cursor_hittest`：继续由现有 `MousePassthrough` 路径切换点击穿透。

旧版本的编译 / smoke 结果不代表后端迁移后的当前提交已验证。先运行 `bash scripts/verify-macos-all.sh`；脚本自动覆盖范围及
仍需人工观察的事项见 [`docs/MACOS_VERIFICATION.md`](docs/MACOS_VERIFICATION.md)。

平台差异仍保持如下：

- Windows：继续用现有 Win32（`GetCursorPos` / `GetAsyncKeyState` / `WS_EX_NOACTIVATE`）。
- macOS：使用 `NSEvent` 和 AppKit；只靠 egui 事件拿不到窗口外的全局光标。
  `winit` 0.30 的 `Window::set_cursor_hittest` 在 macOS 可用，缺的是全局坐标/按键，不是穿透 API。
- `NSEvent::mouseLocation` 是屏幕坐标，原点/Y 方向与 winit 的坐标约定不同；多显示器下要通过
  winit 的 monitor 几何做映射，别直接混用。

验收结论以 Mac 上的完整门禁和人工清单为准；Windows 交叉编译不能替代 Apple SDK 下的实际检查。

### 2. 日常可用性（发布形态）

基础发布能力已经补齐，仍需目标机器做最终验收：

- **单实例**：`crates/petsona-runtime/src/instance_lock.rs` 使用数据目录锁文件；同一
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
- **自动化发布结构检查**：`scripts/verify-macos-all.sh` 在临时目录生成 app/zip 并检查可执行文件、图标、Bundle ID、`LSUIElement`、Retina 字段和 LaunchAgent 模板；不会安装 LaunchAgent，也不替代真实 Finder/Gatekeeper 验收。

### 3. 重绘预算（省电）

- `PetsonaApp::schedule_repaint` 按动画帧时长、气泡、点击判定、拖拽/自动行走和交互轮询安排
  下一次重绘；idle 不再无条件按 60 FPS 重绘。
- 状态协议和后台问候完成时会主动 wake event loop；全局鼠标在需要像素穿透/转头时保留低频兜底轮询。
- 低频指针采样和 smoke 门槛已完成；仍需在 Activity Monitor 实测 B11，确认目标机器空闲 CPU 从原先 8–10% 降到接近 0–1%。

> 原先“偶发闪 / 气泡独立窗口 / 菜单物理像素定位 / 记住位置 / 重力开关”那批 Windows 待办
> 已从本文件移除；需要时从 git 历史或对话里找回。

## 已踩过的坑（别再重新推导）

除特别标注外都是 Windows 结论：

- **拥有窗口的线程必须抽消息（2026-09-16 踩坑）**：菜单 owner 改成真正的顶层窗口后，UI 线程在
  `style_popup_window` 里对同进程**其它线程**的窗口调用 `GetWindowTextW`（同步 `SendMessage`）
  会永久阻塞——那一刻菜单线程正阻塞在 `mpsc::recv()`，不抽消息。修法有两条，缺一不可：
  ① `style_popup_window` 先按 `GetWindowThreadProcessId` 返回的**线程 id** 过滤（只动本线程窗口，
  子类化表本来就是 thread_local）；② 菜单线程跑 `GetMessageW` 循环 + `WM_APP+1` 唤醒。只按进程 id
  过滤不够，`EnumWindows` 会枚举到同进程其它线程的顶层窗口。
- **Win32 弹出菜单要能 Esc/点外关闭**：owner 必须是**能成为前台的顶层窗口**（`HWND_MESSAGE`
  不行、带 `WS_EX_NOACTIVATE` 也不行），弹出前 `SetForegroundWindow(owner)`，结束后
  `PostMessageW(owner, WM_NULL)`，再把前台还给原来那个窗口（所以 B7 不受影响）。否则 Esc 无效、
  点菜单外也不消失。
- **菜单连点会排队叠加**：菜单模态循环期间新请求只能排队，逐个弹。修法：`show()` 里若菜单已打开
  先 `PostMessageW(WM_CANCELMODE)` 取消当前菜单；菜单线程每次关闭后继续消费队列、只取最新位置。
  自动化守卫：smoke `T3`（弹出 → 再弹仍只有 1 个 → 关闭）。
- **气泡/输入框"从侧面滑进来"是 DWM 的显示过渡（2026-09-16 更正，之前判断错了）**：
  窗口本身逐帧几何完全不动（实测 30 帧坐标不变），侧面滑动/淡入来自 Windows 在 `ShowWindow`
  时给窗口做的 DWM 过渡；开启"淡入或滑动菜单"辅助选项时更明显，带 `WS_EX_TOOLWINDOW`
  的弹窗尤其明显。两条修法缺一不可：
  ① 设置 `DwmSetWindowAttribute(hwnd, DWMWA_TRANSITIONS_FORCEDISABLED = 3, TRUE)`
  （`petsona-shell-windows/src/platform.rs` 的 `disable_window_transitions`）；
  ② **必须在窗口第一次显示之前设置**——所以气泡和对话窗口都改成"先隐藏创建（warm-up）→
  外壳设属性 → 下一帧才显示"（`bubble_window_warmed` / `conversation_window_warmed`）。
  该属性**读不回来**（`DwmGetWindowAttribute` 返回 `E_INVALIDARG`），自动化靠
  `PlatformHost::popup_transitions_disabled()` 探针 + smoke B9 断言。
- **进场动画是我们自己画的、只动内容不动窗口**（`app.rs`）：
  气泡 `BUBBLE_ENTRY` 160ms，内容从下方 8px 升到位（`BUBBLE_ENTRY_RISE`）；
  对话窗口 `CONVERSATION_ENTRY` 180ms，内容从上方 8px 落到位（`CONVERSATION_ENTRY_DROP`）。
  气泡窗口因此把底部内边距加了 8px（`BUBBLE_BOTTOM_PADDING`），窗口位置相应上移
  （`BUBBLE_WINDOW_GAP = BUBBLE_GAP - BUBBLE_ENTRY_RISE`），保证静止位置与旧版一致。

- **V2 注视会压过 `running-left` / `running-right`（2026-09-16 踩坑）**：引擎里 look-row-9/10
  优先级 20、locomotion 只有 10，而且注视是"光标不离开就不放"的保持状态；所以鼠标停在宠物上时，
  协议发的 `running-left` 会被注视盖住（引擎单测
  `gaze_does_not_override_higher_priority_events_or_get_replaced_by_motion` 固定了这个设计）。
  smoke 的 A8 因此**先把光标移到对面角落**再跑协议状态。
- **转向姿态会让"命中像素"变成穿透（2026-09-16 修复）**：光标移到宠物上 → 宠物转头 → 当前帧的
  不透明像素变了，原来的射线点变透明，窗口立刻切成穿透，于是"看向你的时候点不到它"。
  修法：`cursor_over_pet` 先测当前帧，再回退到 idle 动画所有帧的并集掩码（身体位置始终可点）。
  回归测试 `a_look_pose_still_keeps_the_resting_body_clickable`。
- **位置记忆必须用物理像素（2026-09-16 修复）**：`WindowPosition` 现在存**物理像素窗口原点**
  （Win32 `GetWindowRect` 坐标系），逻辑点只在 winit 边界换算；旧版本存的逻辑坐标会被还原路径
  夹回工作区，不会跑到屏幕外。工作区来自 `PlatformHost::monitor_work_area`
  （Windows = `MonitorFromPoint` + `GetMonitorInfoW(rcWork)`）。
- **开机自启写的是 HKCU Run，受限会话会 ACCESS_DENIED**：注册表值名默认 `Petsona`，
  可用 `PETSONA_AUTOSTART_VALUE_NAME` 覆盖（smoke 用一次性名字，绝不碰用户真实启动项）。
  在沙箱/CI 里 `RegCreateKeyW` 可能返回错误码 5，此时 smoke T4 会明确跳过而不是误报失败。
- **PowerShell 5.1 的 `Compress-Archive` 会写反斜杠条目名**（zip 规范要求 `/`）。打包改用
  `[System.IO.Compression.ZipFile]::CreateFromDirectory(...)`；校验条目时把 `\` 归一化成 `/`。
- **`#![windows_subsystem]` / exe 图标**：图标靠 `rc.exe`（MSVC）或 `windres.exe`（GNU）编译
  `packaging\windows\Petsona.rc`，由 `crates\petsona-shell-windows\build.rs` 通过
  `cargo:rustc-link-arg-bins` 链接；不要为此引入 winresource/embed-resource 依赖。
- **winit 的 `decorations(false)` 不是真的无边框**：窗口仍带 `WS_CAPTION | WS_BORDER | WS_DLGFRAME |
  WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX`，只靠 `WM_NCCALCSIZE` 装样子。任何状态变化
  （激活、切样式、改尺寸）都会让 Windows 重画那个边框——这就是用户反复报告的“闪现”。
  做法：`SetWindowLongPtrW(GWL_STYLE)` 去掉这些位、加上 `WS_POPUP`，再 `SetWindowPos(SWP_FRAMECHANGED)`；
  随后必须重新 `DwmEnableBlurBehindWindow`（空 blur 区域）恢复透明，因为 frame 重算会把 winit 设的透明弄丢。
  实测：`style=0x96000000 POPUP=True CAPTION=False`。
- **不要每帧重复发 `ViewportCommand::InnerSize` / `WindowLevel`**：会反复 `SetWindowPos` → 闪。
  只在该变的时候发（`app.rs` 里有 `applied_window_size` / `applied_always_on_top` 缓存）。
- **`tray-icon` 的原生菜单会卡死 Windows 程序**：Windows 的 `TrackPopupMenu` 在事件循环线程上开模态循环，
  期间宠物不重绘、“退出”也发不出去，所以 Windows 现在由专用 Win32 菜单线程承载；macOS 使用 AppKit 原生菜单。
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
  `PETSONA_HOME` 覆盖（测试 / 便携用）。宠物只有**本地库** `...\Petsona\pets` 会被加载：
  2026-09-16 起不再自动扫描 `~/.codex/pets` / `~/.unipet/pets`，而是在设置 →「宠物」→
  「从 Codex 导入」里列出 `~/.codex/pets` 的内容，用户点「导入」后才复制进本地库
  （`PetLibrary::discover` 只返回本地根；`codex_pets_dir()` + `PetLibrary::scan_dir()`
  只读列目录）。
- 内置宠物（2026-09-16 更换）：**Superintendent** by Renner Campos，V2 8×11 包
  （id `Superintendent_Petdex`，图集 1536×2288，内嵌在 `crates/petsona-core/assets/default-pet/`）。
  旧的 `petsona-default`（ByteBot）不再随包发布；老用户本地残留的副本不会被删除。
- 位置记忆：`config.json` 的 `window.startPosition` 是**物理像素**；`window.gravityEnabled`
  默认 `false`（重力 2600 px/s²、上限 1800 px/s，落到工作区底部播放一次 `jumping`）。
- 开机自启：`HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 下 `Petsona = "<exe 路径>"`；
  测试可用 `PETSONA_AUTOSTART_VALUE_NAME` 换一个值名。设置 →「启动」里的开关读的是注册表真实状态。
- 状态协议：`127.0.0.1:17872`，`POST /state`（`{source,state,message,ttlMs}`）、`GET /health`、
  `GET /pets`；`ttlMs: 0` 表示不过期。状态名见 `docs/PET_NATIVE.md`。
  发行版宠物实测：V2 = 8×11；**idle 画了 7 帧**（官方时长表写 6），引擎按“真正画了内容的格子”取帧数；
  第 9 / 10 行是“转过去再转回来”的动作，持续注视时拆为转向/保持/返回，实测**第 9 行 = 转向右，第 10 行 = 转向左**
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

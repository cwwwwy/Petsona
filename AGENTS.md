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

## scripts 目录约定（2026-09-16 起保持精简）

- 每端只有一个验收入口：`verify-windows.ps1` / `verify-macos-all.sh`。**不要**再新增 `.cmd` / `.command` 包装。
- 长流程 smoke（`windows-smoke.ps1`、`macos-smoke.sh`）允许独立成文件：它们可以对已构建产物单独重跑，
  由入口脚本调用。
- 打包各一份：`package-windows.ps1` / `package-macos.sh`。
- 资源生成器放资源旁边（如 `packaging\windows\generate-icon.ps1`），不要堆进 `scripts\`。
- 新增脚本前先问：能不能并进入口脚本或已有脚本？（2026-09-16 已把 macos-package-smoke.sh 并进入口、
  把 windows-smoke-native.cs 内嵌进 smoke、把图标生成器移到 packaging\windows\。）

## 两端唯一验收入口（2026-09-16 收束）

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

## 当前状态（2026-09-16）

- 项目已从 BytePet 改名为 **Petsona**；本机仓库目录为 `C:\Users\happyddz\Desktop\Petsona`
  （原 `Desktop\bytepet` 已不存在）。**GitHub 仓库已于 2026-09-16 重命名为
  `https://github.com/cwwwwy/Petsona.git`**（旧 `bytepet` 地址由 GitHub 自动重定向，但每台机器都要
  `git remote set-url origin https://github.com/cwwwwy/Petsona.git` 或 `gh repo rename` 更新一次）。
- 当前分支 `codex/cross`（已推送 origin），比 `main` 多 7 个提交；最新提交是
  `5619f84` `feat: add pet scale presets and conversation UI`。`main` 在 `af99831`（跨平台 verify 脚本）。
- 平台架构决策（2026-09-16）：采用「共享 `petsona-core` / `petsona-runtime` + 独立平台 UI 外壳」；Windows 和 macOS 可以独立发布和独立规划，但不拆仓库、不维护长期平台开发分支。目标见 `docs/PLATFORM_ARCHITECTURE.md`。
- 最近一次工作区测试（Windows，2026-09-16）：core 56 + app 8（`test-hooks` 下 10）+ runtime 1 通过；
  macOS 外壳自带 3 个几何/坐标换算单测（从 app 搬过去），mac 上跑 workspace 测试时会执行。
  历史记录：Mac smoke 曾通过 31 项（Key Window、低频指针采样、idle 重绘、固定缩放几何、位置保存、
  对话流程、CPU 采样）；bundle/zip/Info.plist/LaunchAgent 静态检查已内联在 `scripts/verify-macos-all.sh`。
- 平台验收：macOS A3、B1–B4、B6–B7 曾实测通过；触控板右键、设置聚焦已修复；固定缩放档位、位置记忆、对话输入/发送、记忆事件和输入框光标驱动注视已实现并由 smoke 覆盖。当前宠物图集的 row9/row10 是 16 个转向目标帧，不是独立的 16 个静态方向；真实注视方向视觉、气泡回复按钮、对话窗口观感和输入时注视仍需实机复验。CPU 仍需 Activity Monitor 复测；多屏/Retina 暂缓；B9、B12/B13、A1/A2/A4–A11、真实签名/公证仍待实测。
- 菜单决策（2026-09-14）：不引入 WinUI3/Windows App Runtime；macOS 托盘/宠物右键继续使用 AppKit 原生菜单，Windows 改用同进程专用 Win32 原生菜单线程，优先保证性能、主题一致性和零额外运行时。
- Windows 自动门禁：`scripts\verify-windows.ps1 -Full` 已于 2026-09-16 在全平台后端搬移后重新全绿
  （fmt / clippy / workspace test / release 链接 / test-hooks clippy+test / release 构建 / smoke 16 项）。
- Windows 实机状态（用户 2026-09-14 更新，详见 `docs/WINDOWS_VERIFICATION.md`）：
  A1–A7、A10–A12 通过，A8/A9 的协议层已由 smoke 自动通过，A13 属长测；B1–B6、B9、B10、B12–B14 通过，
  B5 的持续注视共享状态机已实现，Windows 实机仍需验证；B7 已加入 `WM_MOUSEACTIVATE -> MA_NOACTIVATE` 守卫，B8 已加入 `WM_STYLECHANGING` 守卫，并修复缩放底部中心锚点与命中掩码缩放；B11 已改为低层鼠标事件唤醒并缓存坐标，真实桌面 CPU/GPU 仍需实机确认；B7/B8 的肉眼/真实输入仍需实机确认；
  C1 通过，C2/C3 待 Phase 2，C4–C5 待实测，C6 的宠物窗口与菜单样式已自动化通过。
- 分支策略（2026-09-16 更新）：`codex/cross` 已快进合并进 `main`，本地和远程分支都已删除；
  现在**直接在 `main` 上开发**（见文末「Git 工作流」）。需要试验性改动时开短期分支，
  合并后立即删除，不长期保留平台分支。
- 改名决策（2026-09-14）：**不提供 BytePet → Petsona 数据迁移**，开发验证阶段接受从零开始，
  不读取旧 config/personas/memory/pets。
- CI：`pull_request` → main、`push` tag `v*`、`workflow_dispatch`；直接推 `main` 不跑；
  同一 ref 的旧运行会被 concurrency 取消。

## 平台架构决策（2026-09-16）

用户已确认：Windows 与 macOS 可以采用独立 UI 外壳，发布节奏也允许不同步；开发精力约为
Windows 70% / macOS 30%。因此项目采用：

- 一个仓库、一个 workspace、共享 `petsona-core` 和 `petsona-runtime`；
- Windows/macOS 各自拥有窗口、菜单、输入、渲染和打包外壳；
- 平台功能可以使用 `codex/win-*` / `codex/mac-*` 短期分支，但不长期维护两套开发主线；
- Windows 和 macOS 使用独立发布 tag / workflow，允许一边领先；
- 共享核心变更仍需保证两边编译和核心测试通过，触及平台能力时跑对应平台 smoke。

当前迁移阶段（2026-09-16）：共享 `petsona-runtime` 已接管核心状态；`petsona-app` 已经成为纯共享
UI 库（不存在任何 `#[cfg(target_os)]`，也不再提供二进制），平台后端全部通过
`petsona_app::platform::PlatformHost` 注入：

- `petsona-shell-windows`：Win32 无边框/不激活样式、`WM_STYLECHANGING` 守卫、
  `WH_MOUSE_LL` 低层鼠标钩子、`SetWindowPos` 几何、explorer 打开目录、Win32 弹菜单线程。
- `petsona-shell-macos`：AppKit 非激活面板、NSEvent/CoreGraphics 全局光标与 Escape、
  NSOpenPanel/NSSavePanel、AppKit 托盘菜单（`show_context_menu_for_nsview`）。

Windows 验收（smoke + `verify-windows.ps1 -Full`）已全部改用 `target\release\petsona-windows.exe`；
macOS 的 `macos-smoke.sh` / `package-macos.sh` 已改用 `petsona-macos`。

发布轨道（2026-09-16 建立）：

- Windows：tag `windows-v<version>` → `.github/workflows/release-windows.yml`（快速门禁 + `scripts\package-windows.ps1` + artifact）。
- macOS：tag `macos-v<version>`（旧的裸 `v<version>` 仍支持）→ `.github/workflows/release-macos.yml`。
- 手动触发两个 workflow 也可以；`ci.yml` 在 `v*` / `windows-v*` / `macos-v*` tag 上跑双平台 fmt/clippy/test。
- 剩余：macOS 实机验证本次搬移、HKCU 自启开关、真实签名/公证。

搬移后的平台接口一览（都在 `petsona_app::platform`）：

- `PlatformHost`：`present_window` / `notify_window_resize` / `set_window_geometry` /
  `set_no_activate_for_title` / `confirm_settings_focus` / `show_context_menu_for_window` /
  `install_event_waker` / `event_driven_mouse` / `throttle_pointer_sampling` / `pointer_snapshot` /
  `escape_pressed` / `create_menu` / `uses_native_tray_menu` / `supports_native_file_dialogs` /
  `choose_pet_import_path` / `choose_pet_export_path` / `open_in_file_manager` +
  `test-hooks` 探针（`cursor_poll_count` / `mouse_event_count` / `mouse_position_valid` /
  `is_window_key_for_title`）。
- `PlatformMenu`：`show` / `poll`，返回 `MenuCommand`（OpenSettings / ChangePet / TogglePet / Quit）。
- `PortableHost`：全默认实现，供 `petsona-app` 自己的单元测试使用。

新增平台能力时的做法：先在 trait 里加一个**带默认实现**的方法（默认行为 = winit/egui 回退），
再在对应外壳里 override。`petsona-app` 不允许再出现 `#[cfg(target_os = ...)]`。

## GitHub 仓库元数据（2026-09-16）

- 仓库名：`cwwwwy/Petsona`（2026-09-16 由 `bytepet` 改名，旧地址 GitHub 自动重定向；
  各机器仍应 `git remote set-url origin https://github.com/cwwwwy/Petsona.git`）。
- About 描述（建议，英文，GitHub 上限 350 字符）：
  `Rust-only desktop pet for Windows & macOS — Codex pet packs, native menus, persona + light memory. No Node/WebView.`
- Topics（建议）：`rust` `desktop-pet` `egui` `eframe` `windows` `macos` `codex` `tray-icon`
- 主页：暂时留空，有官网/演示页再填。
- 修改方式（两种都行）：GitHub 网页仓库首页 About 区域 → 齿轮 → 填描述/主页/Topics → Save；
  或命令行 `gh repo edit --description "..." --add-topic rust --add-topic desktop-pet ...`。
- 已核对与 Petsona 口径一致、无需改动的地方：README 标题与首段、各 crate 包名（`petsona-*`）、
  数据目录（`%APPDATA%\Petsona` / `~/Library/Application Support/Petsona`）、macOS bundle id
  （`com.petsona.desktop`）。`legacy/` 里保留旧 BytePet 代码作为参考，不做改名。

## Windows 开发计划（2026-09-14，修订 2）

当前实机状态以 `docs/WINDOWS_VERIFICATION.md` 为准。下一步先在 `codex/cross` 上做自动化骨架
和基线修复，不提交 PR、不合并 `main`；合并时机由用户决定。

### Phase 0.5 — 自动化验收骨架（第一版已完成）

目标是让日常改动不再逐条手点：

- `scripts\verify-windows.ps1` 快速门禁和 `-Full` 完整门禁均已实现：fmt / clippy / test /
  release build 不启动 GUI；`-Full` 额外调用 `scripts\windows-smoke.ps1`。
- `scripts\windows-smoke.ps1` 已实现，用临时 `PETSONA_HOME` 启动 release，当前自动通过基础检查，另含原生菜单线程就绪检查：
  T1、A1、A4（设置，部分）、A8、A9、A11（部分）、A13（加速，部分）、B9（几何，部分）、
  B12–B14、C6（宠物窗口与菜单样式）。
- `test-hooks` 已实现并默认关闭：本机控制端口提供窗口/独立气泡/注视/重绘/指针轮询/样式重应用快照，
 以及打开菜单和设置、隐藏/显示、独立气泡、缩放、穿透、自动行走、保存、退出等确定性动作。
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
4. **B5 持续注视（共享代码已完成）**：row9/row10 现在拆为转向、保持最强方向帧、返回三个阶段。
   光标进入左右触发区后停在最强方向帧，未离开触发区时保持；离开或回到死区后播放返回段并回 base。
   核心状态机单测已补，下一步用 macOS 实机和 Windows smoke 分别验证全局光标路径。

### Phase 1 — 窗口 + 气泡（代码已完成）

气泡已独立成透明、点击穿透、`with_active(false)`、无边框、置顶的窗口，固定在宠物上方并跟随移动，
无气泡时完全隐藏；宠物窗口已去掉气泡预留高度并收缩到精灵精确尺寸；保持 `WS_POPUP` +
DWM 透明 + `WS_EX_NOACTIVATE`，不在像素级切换穿透路径中重新引入 frame 重算。

### Phase 2 — 多屏 + 位置记忆

菜单用 `GetCursorPos` + `MonitorFromPoint` + `GetMonitorInfoW(rcWork)` + `SetWindowPos`
物理像素落位；拖动结束把物理坐标写入 `window.startPosition`；启动精确还原，保存的显示器不存在时
回落；首次/重置位置放主屏工作区右下角（约 32px）。C2/C3 完成后必须由 smoke 自动检查坐标。

### Phase 3 — 重力开关

设置 → 宠物行为，默认关；约 2600 px/s²、上限约 1800 px/s；落到当前显示器工作区底部并播放
一次 `jumping`；拖动或自动行走时不生效。

### Phase 4 — Windows 发布形态（2026-09-16 大部分已完成）

- ✅ `packaging/windows/Petsona.ico`（16/24/32/48/64/128/256，PNG 条目），由
  `packaging\windows\generate-icon.ps1` 从 `packaging\macos\Petsona.icns` 的 PNG 分块生成，
  两边图标同源；改图标只改 icns 再重跑生成脚本。
- ✅ exe 图标嵌入：`crates\petsona-shell-windows\build.rs` 直接调用 `rc.exe`（MSVC）或
  `windres.exe`（GNU），**不新增 crate**；两者都找不到时只打印 warning 并跳过。
  MSVC 与 GNU 两条工具链均已实测 exe 带图标。
- ✅ `scripts\package-windows.ps1`：出 `dist\Petsona-windows-<arch>-<version>.zip`
  （含 `Petsona.exe`、`Petsona.ico`、`VERSION.txt`、`README.txt`），支持 `-Architecture arm64`、
  `-SkipBuild`、`-OutputDirectory`；zip 由 .NET `ZipFile::CreateFromDirectory` 生成。
  `verify-windows.ps1 -Full` 末尾会自动跑一次打包结构检查（D3）。
- ✅ `.github/workflows/release-windows.yml`：tag `windows-v*` 或手动触发 → 快速门禁 + 打包 + artifact。
- ⏳ 未做：HKCU Run / Startup 自启开关（设置里目前没有开关项）。

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

当前实现：

- macOS 托盘菜单增加了可动态刷新的“选择宠物”子菜单，并使用原生文件面板。
- Windows 托盘菜单和宠物右键菜单已切换到专用 Win32 菜单线程；egui 菜单仅作为线程启动失败时的回退。
- WinUI3 仅作为未来独立 spike，不进入主构建。
### Phase 5 — 发布质量

Fast + Full 验证全绿；手工清单只剩视觉/真实桌面/多屏/DPI/登录自启/Defender；菜单工作区夹取
和位置回退写成纯函数并加单测；Windows 不重写现有 Win32 后端，只做增量。

## 阶段 7 计划（2026-09-16 拟定，待执行）

平台拆分（阶段 5）和 Windows 发布形态（Phase 4）已基本收口，剩下的都是「功能补齐」或
「必须人工确认」的项。按优先级：

1. **D5 开机自启开关（Windows，小）**：设置里加「开机自启动」开关（默认关），写/删
   `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`；启动时以注册表真实状态为准回填 UI。
   用 windows-sys 的 `Win32_System_Registry`（只加 feature，不加 crate）。smoke 走 test-hooks
   开关并检查注册表项出现/消失。
2. **Phase 2 多屏 + 位置记忆（C2/C3，Windows 优先）**：拖动结束保存**物理像素**坐标
   （现在存的是逻辑坐标，混合 DPI 多屏下会跑偏）；启动还原时若目标显示器已不存在则回落到
   最近可见工作区；菜单落位改用 `MonitorFromPoint` + `GetMonitorInfoW(rcWork)` 夹取。
   夹取/回落写成纯函数 + 单测；smoke 自动检查窗口坐标在可见工作区内。
3. **Phase 3 重力开关**：设置 → 宠物行为，默认关；约 2600 px/s²、上限约 1800 px/s，
   落到当前显示器工作区底部并播放一次 `jumping`；拖动或自动行走时不生效。
4. **人工确认包（一次真实桌面会话，10 分钟）**：B5 持续注视、B7 真实点击不抢前台、
   B8 无边框闪（肉眼）、B10 菜单打开时宠物不冻结且退出立即生效、C4 DPI、
   C5 全屏/虚拟桌面、D1 release 无控制台、D6 干净用户环境。
5. **首次发布**：定版本号（当前 0.1.0）→ 打 `windows-v*` tag 跑
   `release-windows.yml`（D4 首次执行）；macOS 拿到 Developer ID 后走 `sign-macos.sh` /
   `notarize-macos.sh`。

macOS 侧（需要用户在 mac 上）：`bash scripts/verify-macos-all.sh` 验证本次平台后端搬移，
再补 B10 多屏、B11 Activity Monitor、A11 keychain、C 节 Retina/Spaces。

## 关键缺口（按优先级）

### 1. macOS 交互后端（核心交互已通过；当前剩余是多屏与功耗实测）

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

代码已经通过编译、clippy、workspace 测试和 release 构建；smoke 已确认原生菜单可创建、当前宠物 checked 状态、设置窗口为 Key Window、低频指针采样、idle 重绘、固定缩放几何、位置保存、对话输入/发送流程和 CPU 采样。
`scripts/macos-smoke.sh` 可自动检查协议、TTL、设置、Key Window、当前宠物 checked 状态、低频指针采样、idle 重绘、固定缩放几何/底部中心锚点、位置保存、CPU 趋势、持久化、独立气泡、对话流程、隐藏/显示和 V2 注视生命周期；app/zip/Info.plist/LaunchAgent 静态结构检查已内联进 `scripts/verify-macos-all.sh`；仍需按
`docs/MACOS_VERIFICATION.md` 实测 B10 多显示器坐标、B11 空闲 CPU，以及 C 节的 Retina/Spaces 行为。

平台差异仍保持如下：

- Windows：继续用现有 Win32（`GetCursorPos` / `GetAsyncKeyState` / `WS_EX_NOACTIVATE`）。
- macOS：使用 `NSEvent` 和 AppKit；只靠 egui 事件拿不到窗口外的全局光标。
  `winit` 0.30 的 `Window::set_cursor_hittest` 在 macOS 可用，缺的是全局坐标/按键，不是穿透 API。
- `NSEvent::mouseLocation` 是屏幕坐标，原点/Y 方向与 winit 的坐标约定不同；多显示器下要通过
  winit 的 monitor 几何做映射，别直接混用。

验收：按 `docs/MACOS_VERIFICATION.md` 的 B 节逐条过。这次搬移是逐行搬运（逻辑未改），但
**Windows 机器无法对 macOS 后端做类型检查**，原因如下：

- `rustup target add aarch64-apple-darwin` 可以装，`cargo check --target aarch64-apple-darwin`
  却会在 `ring`（`ureq` 的 TLS 后端）处失败：它的 C 代码需要 macOS 的 C 工具链，
  Windows 上没有 clang/macOS SDK，`cc` 会直接报 `unrecognized command-line option '-arch'`。
- 所以在 Windows 上只能保证「语法正确 + rustfmt 通过」；macOS 后端改动必须在 mac 上
  `bash scripts/verify-macos-all.sh`（它包含 workspace 的 fmt/clippy/test/release 构建，
覆盖 `cargo check -p petsona-shell-macos`）才算验证。

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
- **气泡是自己画的 egui 独立窗口，不是 Win32 动画**：进场用 140ms ease-out 淡入
  （`app.rs` 的 `BUBBLE_FADE` + `bubble_shown_at`）。要更顺的话下一步是让气泡窗口常驻（像菜单那样
  先建后用），避免文字出现那一帧才建窗口。

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
  `PETSONA_HOME` 覆盖（测试 / 便携用）。本地宠物库在 `...\Petsona\pets`，另有只读引用
  `~/.codex/pets`、`~/.unipet/pets`；同 id 时本地库优先。
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
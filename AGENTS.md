# Petsona 项目须知（给 AI 协作者）

## 项目速览

Petsona 是 Windows / macOS 桌宠：**Rust-only**（eframe/egui + winit + tray-icon + ureq），
无 Node / WebView / Tauri。读取 Codex 宠物包（`pet.json` + 8×9 / 8×11 图集），
播放官方动画，支持人格、轻量 JSON 记忆、DeepSeek 短问候和本地状态协议。

平台现状：**Windows 是主要实测平台**；macOS 后端已接入，仍需在 mac 上人工验收
（`docs/MACOS_VERIFICATION.md`）。CI 绿灯 ≠ macOS 可用。

## 目录

| 路径 | 内容 |
|---|---|
| `crates/petsona-core/` | 宠物格式与动画引擎、人格、记忆、DeepSeek、状态协议 |
| `crates/petsona-runtime/` | 平台无关运行时：配置、宠物会话、实例锁、日志、问候 |
| `crates/petsona-app/` | 共享 egui UI + `PlatformHost` 边界；UI 已拆入 `src/app/` 子模块 |
| `crates/petsona-shell-windows/` | Win32 外壳（bin `petsona-windows`） |
| `crates/petsona-shell-macos/` | AppKit 外壳（bin `petsona-macos`） |
| `docs/` | 平台架构与两端实机验收清单 |

## 常用命令

```powershell
cargo run -p petsona-shell-windows        # Windows 产品入口
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

验收入口（每端只有一个脚本，已包含 fmt / clippy / test / release / smoke / 打包结构检查）：

```text
Windows 快速: powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1
Windows 完整: powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1 -Full   # 20 项 smoke
macOS   完整: bash scripts/verify-macos-all.sh
macOS   门禁: bash scripts/verify-macos-all.sh --gates-only
```

- `scripts\windows-smoke.ps1` / `scripts\macos-smoke.sh` 是 smoke 实现文件，只在定位失败时单独跑。
- 打包：`scripts\package-windows.ps1` / `scripts\package-macos.sh`；资源生成器放资源旁边。
- 不要新增 `.cmd` / `.command` 包装；新增脚本前先问能否并入现有脚本。
- 窗口、托盘、菜单、穿透的肉眼部分仍按 `docs/*_VERIFICATION.md` 检查。

macOS 需要 Xcode Command Line Tools；Windows 需要 VS Build Tools（含 C++ 桌面开发）。
MSVC 缺 `link.exe` 的 GNU 回退写在 `docs/WINDOWS_VERIFICATION.md`。

## 协作偏好（硬规则）

- 用中文交流。
- **Git 只做只读查询，不代替用户操作**：`status` / `log` / `diff` / `remote -v` / `branch -vv` 可以；
  `add` / `commit` / `push` / 建删分支 / tag / 改 remote / `gh repo edit` 一律给命令让用户自己跑，
  除非用户明确说“你来操作”。
- `AGENTS.md` 由 AI 直接维护；重要决策、计划、坑要及时写回本文件。
- 新会话先读：`AGENTS.md`、`docs/PLATFORM_ARCHITECTURE.md`、`docs/WINDOWS_VERIFICATION.md`、
  `docs/MACOS_VERIFICATION.md`、git log。
- 尽量不新增第三方依赖：能用标准库 / 现有 `windows-sys` 解决就不加 crate
  （macOS 全局指针 `NSEvent` 是既定例外）。
- 改动大时先说思路；改完至少跑 fmt / clippy / test，平台相关改动跑对应完整验收脚本。
- 仓库对外元数据（GitHub 名称、About、Topics、README 首段）要和 Petsona 同步，建议值见下文。
- 涉及窗口 / 托盘 / 菜单的改动，交付时写清“需要用户实机确认什么”。

## 当前状态（2026-09-17）

- 仓库：`C:\Users\happyddz\Desktop\Petsona`；分支 `main`。当前工作区包含本轮 UI 修复与项目清理改动，
  提交 / 推送仍由用户手动执行。
- 最新提交：`67f839d` `docs: trim redundancy, drop PET_NATIVE/WINUI3 notes`；此前功能提交为
  `81a3fe4`（阶段 7 的 1/2/3 已完成并推送；`codex/cross` 已合并删除）。
- 工作区进行中（2026-09-17，未提交）：修复 Win32 菜单打开设置后焦点回跳；输入框改为单行输入 + ↑，
  去掉标题 / 历史 / 关闭按钮 / spinner，Esc 关闭；输入框从宠物影子放大、关闭缩回影子；气泡去掉独立
  “回复”按钮，改为点击气泡本体。Rust 门禁与 Windows 20 项 smoke 已通过。
- 项目清理已完成（2026-09-17，未提交）：`legacy/` 旧 Tauri 工程已从工作区删除，历史仍保留在 Git 中；
  `dist/` 与 `target/` 本地产物已清理。`petsona-app` 完成纯移动式模块化：`app.rs` 4,704 → 1,143 行；
  功能拆到 `app/{bubble,conversation,settings,interaction,menus,pets,test_hooks,geometry}.rs`。
  清理后 Rust 门禁与 Windows 20 项 smoke 均通过。
- GitHub 仓库：`https://github.com/cwwwwy/Petsona.git`（2026-09-16 由 bytepet 改名；
  旧地址自动重定向，其他机器仍需 `git remote set-url origin ...` 更新一次）。
- 测试基线：`cargo test --workspace` 全绿（core / app / runtime / shell-windows；
  macOS 外壳另有几何单测，mac 上跑）。
- Windows 自动化：`verify-windows.ps1 -Full` 全绿，20 项 smoke：T1–T4、A1/A4/A8/A9/A11/A13、
  B7–B9/B11–B14、C3/C6/C7；受限会话无法写 HKCU 时 T4 明确 `[SKIP]`。
- Windows 实机：A1–A7、A10–A12、B1–B6、B9、B12–B14、C1 已通过（B7 自动化 + 真实 SendInput 通过）；
  B5/B8/B10/B15/C4/C5 与 H1–H5 仍需实机确认，细节以 `docs/WINDOWS_VERIFICATION.md` 为准。
- macOS 实机：A3、B1–B4、B6–B7 已通过；多屏 / Retina、Activity Monitor、签名 / 公证待验收
  （`docs/MACOS_VERIFICATION.md`）。

### 阶段 7 剩余

1. **人工确认包**：Windows 的 H1–H5（登录自启、副屏位置、副屏菜单、拔屏回落、重力手感），
   加上已有的 B5 / B7 / B8 / B10 / C4 / C5 / D1 / D6。
2. **首次发布**：定版本 → 打 `windows-v*` tag 跑 `release-windows.yml`（D4 首次执行）；
   macOS 拿到 Developer ID 后走 `sign-macos.sh` / `notarize-macos.sh`。
3. **macOS 补课**（需要在 mac 上）：`bash scripts/verify-macos-all.sh`，再补 B10 多屏、
   B11 Activity Monitor、A11 keychain、C 节 Retina/Spaces。

## 架构约定

- 一个仓库、一个 workspace、共享 `petsona-core` / `petsona-runtime`；Windows 与 macOS 各自拥有
  UI 外壳，可独立发布（tag `windows-v*` / `macos-v*`），不长期维护平台分支。
- `petsona-app` 是纯共享库：不允许 `#[cfg(target_os = ...)]`，也不提供二进制。
- 平台能力全部经 `PlatformHost` 注入，每个方法都有可移植默认实现（winit / egui 回退）。
  新增能力 = 在 trait 加带默认实现的方法 + 在对应外壳 override。
- `PhysicalRect` 是跨混合 DPI 的唯一几何单位（物理像素）；逻辑点只在 winit 边界换算。
- Windows 菜单用进程内 Win32 菜单线程，macOS 用 AppKit 原生菜单；不接受 WinUI3 /
  Windows App SDK 依赖（决策见 `docs/PLATFORM_ARCHITECTURE.md`）。
- 接口清单与迁移历史见 `docs/PLATFORM_ARCHITECTURE.md`。

## GitHub 元数据

- 仓库：`cwwwwy/Petsona`（About / Topics 已于 2026-09-16 设置）。
- 改名 / 改定位后同步：GitHub About + Topics、README 首段、crate 包名、数据目录、
  macOS bundle id（`com.petsona.desktop`）。

## 关键事实速查

- 数据目录：`%APPDATA%\Petsona` / `~/Library/Application Support/Petsona`，可用 `PETSONA_HOME` 覆盖。
- 宠物库：只加载本地 `...\Petsona\pets`（`PetLibrary::discover()` 只返回本地根）；
  `~/.codex/pets` 只作为「从 Codex 导入」的来源（`codex_pets_dir()` + `PetLibrary::scan_dir()`），
  不再自动扫描 `~/.unipet/pets`。内置宠物：Superintendent by Renner Campos
  （id `Superintendent_Petdex`，V2 8×11，1536×2288）；在设置里删除可永久停用重装。
- 配置：`config.json`。`window.startPosition` 是**物理像素**；`window.gravityEnabled` 默认 `false`
  （重力 2600 px/s²、上限 1800 px/s，落到当前显示器工作区底部并播放一次 `jumping`）。
- 开机自启（Windows）：`HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 的 `Petsona` =
  `"<exe 路径>"`；测试可用 `PETSONA_AUTOSTART_VALUE_NAME` 换一个值名。
- 状态协议：`127.0.0.1:17872`，`POST /state`、`GET /health`、`GET /pets`；`ttlMs: 0` 表示不过期。
- 宠物帧数按图集实际绘制推断（发行版 V2 的 idle 实际画 7 帧，官方表写 6）；row9 = 右转、
  row10 = 左转（依据与测量方法见 `pet/state.rs` 注释和 `pet_inspect`）。
- 日志：数据目录 `logs/petsona.log`；单实例锁：数据目录 `petsona.lock`。
- release profile：`lto = "thin"`、`codegen-units = 1`、`strip = true`、`panic = "abort"`。
- Windows 打包产物：`dist\Petsona-windows-x64-<version>.zip`（含 exe、图标、VERSION、README）；
  exe 图标由 `crates\petsona-shell-windows\build.rs` 调 `rc.exe` / `windres.exe` 编译
  `packaging\windows\Petsona.rc`，不引 crate。

## 坑与教训（别再重新推导）

- **拥有窗口的线程必须抽消息**：跨线程 `GetWindowTextW` 是同步 `SendMessage`，会永久阻塞。
  `style_popup_window` 只处理本线程窗口 + 菜单线程跑 `GetMessageW` 循环（`WM_APP+1` 唤醒），缺一不可。
- **Win32 菜单要能 Esc / 点外关闭**：owner 必须是能成为前台的顶层窗口；弹出前
  `SetForegroundWindow(owner)`，结束后 `PostMessage(WM_NULL)` 并把前台还给原窗口。B7 因此不受影响。
- **菜单连点会排队叠加**：`show()` 里若菜单已打开先 `PostMessage(WM_CANCELMODE)`，
  菜单线程每次关闭后只取最新请求。守卫：smoke T3。
- **气泡 / 输入框“从侧面滑入”是 DWM 显示过渡**：必须在窗口第一次显示前设置
  `DWMWA_TRANSITIONS_FORCEDISABLED`（先隐藏创建 warm-up → 设属性 → 再显示）。该属性读不回来，
  自动化靠探针 + smoke B9。
- **进场动画是自绘的**：气泡从下往上 8px / 160ms；输入框从宠物脚下的阴影放大，
  关闭时缩回阴影（240ms / 200ms）。不再通过移动 / 改变窗口几何做输入框动画。
  气泡底部 `BUBBLE_BOTTOM_PADDING` 与窗口 `BUBBLE_WINDOW_GAP` 是成对常量，改一处必须改另一处。
- **输入框是单一轻量组件**：只保留单行输入和向上箭头发送按钮；没有标题、历史、关闭按钮或 spinner。
  Esc 关闭；点击气泡本体（不是额外的“回复”按钮）或双击宠物打开。组件由原生透明窗口承载但仍由 egui
  自绘，不引入 Win32 `EDIT` / WinUI3 子控件，以保留透明和缩放动画。
- **Win32 菜单打开设置时不能恢复旧前台**：菜单线程在 `打开设置` / `更换宠物` 命令后跳过
  `SetForegroundWindow(previous)`；`WindowsHost::confirm_settings_focus` 在设置窗出现后重新确认前台，
  避免“聚焦后立刻失去焦点”。后台脚本会话受 Windows 前台锁限制，焦点观感仍需实机确认。
- **V2 注视优先于 locomotion**：look-row-9/10 优先级 20 > running-left/right 的 10，且“光标不离开就不放”。
  鼠标停在宠物上时，协议发的 `running-left/right` 会被注视盖住（设计如此，有单测）；
  smoke A8 先把光标移到对面角落再跑协议状态。
- **转向姿态会让命中像素变透明**：光标移到宠物上 → 它转头 → 当前帧像素变了 → 窗口变穿透 → 点不到。
  修法：`cursor_over_pet` 先测当前帧，再回退 idle 全帧并集掩码；回归测试
  `a_look_pose_still_keeps_the_resting_body_clickable`。
- **位置记忆必须用物理像素**：逻辑点在混合 DPI 桌面会漂。工作区 Windows 用
  `MonitorFromPoint` + `GetMonitorInfoW(rcWork)`（纯函数 `clamp_rect_to_work_area` /
  `monitor_for_point` + 单测），显示器不存在时回落到最近可见工作区；
  macOS 暂时回落到 winit 整块显示器边界（Dock / 菜单栏未排除）。
- **HKCU Run 在受限会话会 ACCESS_DENIED（错误 5）**：smoke T4 此时明确 `[SKIP]`，不误报失败。
- **winit `decorations(false)` 不是真无边框**：窗口仍带 `WS_CAPTION | WS_BORDER | WS_DLGFRAME |
  WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX`，任何状态变化都会画出边框（就是“闪现”）。
  修法：去掉这些位 + `WS_POPUP` + `SetWindowPos(SWP_FRAMECHANGED)`，再重新
  `DwmEnableBlurBehindWindow` 恢复透明。实测 `style=0x96000000`。
- **子 viewport 不会经过根窗口的 `present_window`**：气泡 / 对话窗要各自处理 Win32 风格。
  对话窗需要键盘焦点，所以用 `prepare_activatable_popup_window` 去边框但保留可激活；
  每帧重新确认，避免 winit patch 鼠标穿透 / 尺寸时把标题栏加回来。smoke B9 已验证。
- **不要每帧重复发 `InnerSize` / `WindowLevel`**：会反复 `SetWindowPos` 导致闪；
  用 `applied_window_size` / `applied_always_on_top` 缓存。
- **`tray-icon` 原生菜单在 Windows 事件循环上会卡死**：Windows 用专用 Win32 菜单线程，
  macOS 用 AppKit 原生菜单。
- **不激活窗口很重要**：宠物 / 气泡 / 菜单都不抢焦点（Windows `WS_EX_NOACTIVATE`，
  macOS 非激活面板），否则会打断用户正在编辑的应用。
- **状态协议偶发空响应的根因**：Windows `accept()` 的 socket 继承监听 socket 的非阻塞模式；
  accept 后显式 `set_nonblocking(false)`（`every_request_gets_a_response` 回归测试）。
- **HTTRANSPARENT 只在同线程窗口可靠转发**：跨进程需要 `WS_EX_TRANSPARENT`，但会引入抖动——
  “像素级穿透”和“零闪烁”要权衡。
- **macOS 只靠 egui 事件拿不到窗口外全局光标**：转头、拖拽、点击穿透都要走 NSEvent / CoreGraphics。
- **PowerShell 5.1**：含中文的 `.ps1` 必须 UTF-8 **BOM**；`Compress-Archive` 会写反斜杠条目名，
  打包改用 `ZipFile::CreateFromDirectory`。
- **Windows 机器无法类型检查 macOS 后端**：`ring` 需要 macOS C 工具链；macOS 改动必须在 mac 上
  跑完整脚本才算验证。


## Git 工作流

```powershell
git fetch --prune
git pull --ff-only
# ...改代码...
git add -A
git commit -m "..."
git push
```

直接在 `main` 上开发；试验性改动可开 `codex/win-*` / `codex/mac-*` 短期分支，合并后删除。
全局配置已设好：`pull.ff=only`、`push.autoSetupRemote=true`、`fetch.prune=true`、`core.longpaths=true`。

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
| `docs/` | 平台架构、两端实机验收清单、Windows 问题跟踪（`WINDOWS_ISSUES.md`） |

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

## 当前状态（2026-09-18）

- 主线基线 `664cde2`（2026-09-19，子窗 1px 非客户区 / 常驻渲染 / 影子缩放）。
- 2026-09-19 未提交（批次 4–7）：注视 16ms 步进、缩放过渡、右键菜单点击守卫、移除内置宠物 + 首启设置窗、
  立即活动菜单、smoke 自绘夹具；代码与本地门禁全绿，待实机复测。
- 决策（2026-09-18）：**Windows 优先**——先把 `docs/WINDOWS_ISSUES.md` 全部问题修完并实机确认，
  再开 Linux 端；Linux 可行性与范围决策要点见该文档附录，本轮不做。
- 已合入：删除旧 `legacy/` 树、`petsona-app` UI 模块化、CJK 字体路径经 `PlatformHost` 注入、
  Windows `autostart.rs` / `no_activate.rs` 分拆、方向姿势注视与影子 / 对话输入框动画。
- 2026-09-17 本轮新增：macOS 设置页 LaunchAgent 开关、以 `NSScreen.visibleFrame` 计算工作区，
  用临时目录 smoke 验证 LaunchAgent plist 开 / 关。
- 2026-09-17 的完整 `bash scripts/verify-macos-all.sh` 曾通过：fmt / clippy / workspace 测试（app 14、core 58、
  runtime 1、macOS shell 7）/ release 构建、34 项运行 smoke 和打包结构检查。当前运行会话没有可枚举的
  winit 显示器，所以 visibleFrame runtime 检查明确 `[SKIP]`；坐标换算单测通过，真实可用区边缘行为与注销后自启仍待实机。
- 2026-09-18 已提交（67c8b7f）：影子中心位于宠物窗口下方 22pt、40pt 交互窗完全避开宠物；编辑按钮与输入框共享
  34pt 锚点并原位横向展开；注视改为宠物附近的椭圆触发区（短边额外留白 25%，退出迟滞 35%），左右换行先经过
  对应的上 / 下边缘姿势；注视姿势间隔与活动采样均为 40ms。fmt / clippy / release 和 workspace 测试通过
  （app 19、core 59、runtime 1、macOS shell 7）；33 项 macOS runtime smoke 与打包结构检查通过。
  当前会话没有可枚举的显示器，NSScreen 工作区与设置 Key Window 两项明确 `[SKIP]`；影子 / 输入框过渡和注视观感仍需人工确认。
- GitHub 仓库：`https://github.com/cwwwwy/Petsona.git`（2026-09-16 由 bytepet 改名；
  旧地址自动重定向，其他机器仍需 `git remote set-url origin ...` 更新一次）。
- 测试基线：`cargo test --workspace` 全绿（core / app / runtime / shell-windows；
  macOS 外壳另有几何单测，mac 上跑）。
- Windows 自动化：`verify-windows.ps1 -Full` 全绿，20 项 smoke：T1–T4、A1/A4/A8/A9/A11/A13、
  B7–B9/B11–B14、C3/C6/C7；受限会话无法写 HKCU 时 T4 明确 `[SKIP]`。
- Windows 实机：A1–A7、A10–A12、B1–B6、B9、B12–B14、C1 已通过（B7 自动化 + 真实 SendInput 通过）；
  B5/B8/B10/B15/C4/C5 与 H1–H5 仍需实机确认，细节以 `docs/WINDOWS_VERIFICATION.md` 为准。
- macOS 实机：A3、B1–B4、B6–B7 曾通过；B5 注视视觉、B8 缩放观感、B9 输入框动画 / caret gaze、
  B11 Activity Monitor、A12 下一次登录启动仍待确认；多屏 / Retina 按用户决定暂缓，签名 / 公证待凭据。

### 阶段 7 剩余

1. **Windows 问题清零（当前唯一主线）**：`docs/WINDOWS_ISSUES.md` 批次 1–7 代码已完成（含移除内置宠物、
   首启设置窗、立即活动、注视流畅、缩放过渡、右键回馈、子窗白线 / 闪框修复）；剩余是**实机复测**与 D 组打包交付。
2. **Windows 首次发布**：清单清零后定版本 → 先 `workflow_dispatch` 试跑 → 再打 `windows-v*` tag；
   发布前必须解决 GitHub Release 通道（W-28）；W-27 已通过移除内置宠物解决；
   macOS 拿到 Developer ID 后走 `sign-macos.sh` / `notarize-macos.sh`。
3. **macOS 人工验收**：确认 A12 LaunchAgent 在下一次登录启动、`NSScreen.visibleFrame` 的窗口夹取 / 重力落点、
   B5 注视视觉、B8 缩放、B9 caret gaze、B11 Activity Monitor、A11 Keychain、真实签名 / 公证；多屏 / Retina 暂缓。

## 架构约定

- 一个仓库、一个 workspace、共享 `petsona-core` / `petsona-runtime`；Windows 与 macOS 各自拥有
  UI 外壳，可独立发布（tag `windows-v*` / `macos-v*`），不长期维护平台分支。
- `petsona-app` 是纯共享库：不允许 `#[cfg(target_os = ...)]`，也不提供二进制。
- 平台能力全部经 `PlatformHost` 注入，每个方法都有可移植默认实现（winit / egui 回退）。
  新增能力 = 在 trait 加带默认实现的方法 + 在对应外壳 override。
- 字体候选路径经 `PlatformHost::cjk_font_candidates` 注入，`petsona-app` 不直接判断操作系统。
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
  不再自动扫描 `~/.unipet/pets`。**无内置宠物**：本地库为空时启动会直接打开设置窗口；
  测试/ smoke 用自绘夹具 `crates/petsona-core/testdata/v2-test-pet`（V2 8×11，无第三方素材）。
- 配置：`config.json`。`window.startPosition` 是**物理像素**；`window.gravityEnabled` 默认 `false`
  （重力 2600 px/s²、上限 1800 px/s，落到当前显示器工作区底部并播放一次 `jumping`）。
- 开机自启（Windows）：`HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 的 `Petsona` =
  `"<exe 路径>"`；测试可用 `PETSONA_AUTOSTART_VALUE_NAME` 换一个值名。
- 开机自启（macOS）：设置页写入 / 删除 `~/Library/LaunchAgents/com.petsona.desktop.plist`；
  `scripts/install-macos-launch-agent.sh` 仍用于手动管理打包的 `.app`。
- 状态协议：`127.0.0.1:17872`，`POST /state`、`GET /health`、`GET /pets`；`ttlMs: 0` 表示不过期。
- 宠物帧数按图集实际绘制推断（发行版 V2 的 idle 实际画 7 帧，官方表写 6）；row9 = 右侧方向
  姿势表、row10 = 左侧方向姿势表，每行中间帧是中性姿势；依据与测量方法见 `pet/state.rs` 和
  `pet_inspect`。
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
- **进场动画是自绘的**：气泡从下往上 8px / 160ms；影子悬停 180ms 变为编辑按钮，
  输入框从按钮位置展开、关闭时缩回（220ms / 180ms）。不再通过移动 / 改变窗口几何做输入框动画。
  气泡底部 `BUBBLE_BOTTOM_PADDING` 与窗口 `BUBBLE_WINDOW_GAP` 是成对常量，改一处必须改另一处。
- **winit 会重设整份窗口样式**：鼠标穿透 / 显示等 flag 变化时，winit 按自己的 flag 重算 GWL_STYLE，把
  `WS_CAPTION` / `WS_THICKFRAME` 加回来（透明子窗闪现一次系统边框）。修法：窗口过程拦截 `WM_STYLECHANGING`
  就地剥掉这些位（frameless 子类也要拦，不只是 NOACTIVATE），配合隐藏 warm-up + 每帧/显示后重申；
  守卫：连续 12 轮开关输入框，可见帧样式必须始终是 `0x96000000`。
- **子窗保留 1px 非客户区（透明窗顶部白线的真根因）**：egui viewport 窗口的客户区原点比窗口低 1px ——
  顶部 1px 被系统画白、底部 1px 被裁（表现为「输入框下方被横切」）。SWP_FRAMECHANGED 与 DWM 边框属性都无效；
  修法：窗口过程 `WM_NCCALCSIZE`（wparam!=0 → 返回 0，客户区=整窗）+ `WM_NCPAINT` → 0，安装子类后
  强制一次 `SetWindowPos(SWP_FRAMECHANGED)`；可激活窗口用 frameless-only 子类（不强制 NOACTIVATE）。
- **egui 子窗不能停止渲染**：未渲染的 viewport 会被销毁，下次显示是新窗口（肉眼即「偶发重启」）。
  子窗必须每帧 `show_viewport_immediate`，隐藏用 `with_visible(false)` + 空内容。
- **按精灵尺寸缩放**：`pet_window_size()` 宽度有 `PET_WINDOW_MIN_WIDTH` 钳制，小档位不缩；
  影子 / 注视这类跟随精灵的量要用 `pet_size()`。
- **Win11 会给顶层窗口画系统边框 / 圆角**：剥掉所有经典 frame 样式后仍有一条 1px 边框（透明子窗上就是顶部白线）。
  修法：`DWMWA_BORDER_COLOR = DWMWA_COLOR_NONE` + `DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_DONOTROUND`，
  宠物 / 气泡 / 影子 / 输入框都要设；旧系统会拒绝属性，失败无害。
- **winit 忽略非 resizable 窗口的运行时 resize**：`.with_resizable(false)` + `.with_inner_size()` 改不动尺寸；
  需要动态尺寸的窗口要一次给最大尺寸、内容在里面缩放。
- **`TextEdit::frame(Frame::NONE)` 会忽略 `.margin()`**：margin 仅默认 frame 生效；撑高用 `.min_size()` +
  `.vertical_align()`。
- **egui 会回收未渲染的 viewport**：气泡 / 影子 / 输入框被隐藏后若停止渲染，下次出现是**新建 Win32 窗口**，
  `warm-up → 设属性 → 显示` 只对首次有效，之后会带一帧默认边框并播放 DWM 滑入。修法：隐藏时重置 warm-up 标志，
  每次重建都重做 warm-up；`style_popup_window` 需同时禁用 DWM 非客户区渲染。
- **输入框是单一轻量组件**：固定小影子始终显示在宠物下方；悬停变为圆形编辑按钮；点击按钮展开
  小型输入框，只保留输入框和向上箭头发送按钮。输入框宽度约 300pt，文字自动换行并增高；
  Enter 发送、Shift+Enter 换行、Esc 关闭。组件由原生透明窗口承载但仍由 egui 自绘，不引入 Win32
  `EDIT` / WinUI3 子控件，以保留透明和缩放动画。
- **Win32 菜单打开设置时不能恢复旧前台**：菜单线程在 `打开设置` / `更换宠物` 命令后跳过
  `SetForegroundWindow(previous)`；`WindowsHost::confirm_settings_focus` 在设置窗出现后重新确认前台，
  避免“聚焦后立刻失去焦点”。后台脚本会话受 Windows 前台锁限制，焦点观感仍需实机确认。
- **V2 注视是方向姿势表**：look-row-9/10 每帧是目标姿势，不是 turn/return 时间线；
  同一行内从中性 / 当前帧沿帧序移动到目标，跨左右行先经过旧行对应的上 / 下边缘姿势，再从新行同侧边缘进入目标；
  触发区是椭圆，半径为宠物半尺寸 + 短边 25% 留白，退出留白 35%，姿势步进 16ms（`Turning`/`Returning` 期间按 16ms 请求重绘），
macOS 活动指针采样 40ms，离开触发区回中性帧。
  优先级 20 > running-left/right 的 10；smoke A8 先把光标移到对面角落再跑协议状态。
- **转向姿态会让命中像素变透明**：光标移到宠物上 → 它转头 → 当前帧像素变了 → 窗口变穿透 → 点不到。
  修法：`cursor_over_pet` 先测当前帧，再回退 idle 全帧并集掩码；回归测试
  `a_look_pose_still_keeps_the_resting_body_clickable`。
- **位置记忆必须用物理像素**：逻辑点在混合 DPI 桌面会漂。工作区 Windows 用
  `MonitorFromPoint` + `GetMonitorInfoW(rcWork)`（纯函数 `clamp_rect_to_work_area` /
  `monitor_for_point` + 单测），显示器不存在时回落到最近可见工作区；
  macOS 通过 `NSScreen.visibleFrame` 排除 Dock / 菜单栏；真实多屏 / Retina 仍待人工验收。
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

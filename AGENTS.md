# Petsona 项目须知（给 AI 协作者）

## 项目速览

Petsona 是 Windows / macOS 桌宠：共享 Rust 核心与运行时，配合各端原生前端（当前 macOS
使用 SwiftUI / AppKit，Windows 目标为 C# / WinUI 3 + Win32）。旧 egui 入口与平台 shell 已删除。读取 Codex 宠物包（`pet.json` + 8×9 / 8×11 图集），
按锁定的 Codex 资源基准播放动画，支持流式聊天、本地历史、偏好与习惯记忆、人格塑造和本地状态协议。

平台现状：**Windows 是主要实测平台**；macOS 后端已接入，仍需在 mac 上人工验收
（`docs/MACOS_VERIFICATION.md`）。CI 绿灯 ≠ macOS 可用。

## 目录

| 路径 | 内容 |
|---|---|
| `crates/petsona-core/` | 宠物格式与动画引擎、人格、记忆、DeepSeek、状态协议 |
| `crates/petsona-runtime/` | 平台无关运行时：配置、宠物会话、实例锁、日志、问候 |
| `crates/petsona-ffi/` | 原生前端 C ABI 雏形，稳定性与生命周期尚未验收 |
| `apps/macos/` | SwiftUI / AppKit 原生 macOS 前端（迁移中） |
| `docs/` | 平台架构、两端实机验收清单、`plans/` `execution/` 跨对话工作流文档 |

## 常用命令

```powershell
dotnet build apps/windows/Petsona.sln -c Release -p:Platform=x64   # Windows 产品入口（原生前端）
cargo build -p petsona-ffi --release
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

验收入口（每端一个脚本；macOS 原生 build/test/smoke/打包已接入，窗口视觉仍需人工）：

```text
Windows 快速: powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1
Windows 完整: powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1 -Full   # 完整原生 smoke
macOS   完整: bash scripts/verify-macos-all.sh
macOS   门禁: bash scripts/verify-macos-all.sh --gates-only
```

- `scripts\windows-smoke.ps1` / `scripts\macos-smoke.sh` 是 smoke 实现文件，只在定位失败时单独跑。
- 打包：`scripts\package-windows.ps1` / `scripts\package-macos.sh`；资源生成器放资源旁边。
- 不要新增 `.cmd` / `.command` 包装；新增脚本前先问能否并入现有脚本。
- 窗口、托盘、菜单、穿透的肉眼部分仍按 `docs/*_VERIFICATION.md` 检查。

macOS 原生工程需要完整 Xcode；修改工程 spec 时需要 XcodeGen，版本与生成工程须同步记录。
Windows 原生前端需要 .NET、Windows SDK 与 Rust MSVC 链接工具。
MSVC 缺 `link.exe` 的 GNU 回退写在 `docs/WINDOWS_VERIFICATION.md`。

## 协作偏好（硬规则）

- 用中文交流。
- **Git 只做只读查询，不代替用户操作**：`status` / `log` / `diff` / `remote -v` / `branch -vv` 可以；
  `add` / `commit` / `push` / 建删分支 / tag / 改 remote / `gh repo edit` 一律给命令让用户自己跑，
  除非用户明确说“你来操作”。
- `AGENTS.md` 由 AI 在获准修改文档时维护长期决策与规则；单次计划和执行证据分别写入下述文档。用户要求只读时不得自行写回。
- 新会话先读：`AGENTS.md`、`docs/PLATFORM_ARCHITECTURE.md`、`docs/WINDOWS_VERIFICATION.md`、
  `docs/MACOS_VERIFICATION.md`、git log。
- 尽量不新增第三方依赖：能用标准库 / 现有 `windows-sys` 解决就不加 crate
  （macOS 全局指针 `NSEvent` 是既定例外）。
- 改动大时先说思路；改完至少跑 fmt / clippy / test，平台相关改动跑对应完整验收脚本。
- 仓库对外元数据（GitHub 名称、About、Topics、README 首段）要和 Petsona 同步，建议值见下文。
- 涉及窗口 / 托盘 / 菜单的改动，交付时写清“需要用户实机确认什么”。

## 跨对话工作流（规划 → 执行 → 审查）

- 计划契约：`docs/plans/<任务名>.md`；执行证据：`docs/execution/<任务名>.md`。模板分别为各目录的 `TEMPLATE.md`。
- 任务线：macOS [native-ui-rewrite](docs/plans/native-ui-rewrite.md)（执行记录 [同名](docs/execution/native-ui-rewrite.md)）；Windows [windows-native-rewrite](docs/plans/windows-native-rewrite.md)（执行记录 [同名](docs/execution/windows-native-rewrite.md)）；跨两端 [settings-consolidation](docs/plans/settings-consolidation.md) 与 [persona-memory-reshape](docs/plans/persona-memory-reshape.md)（一宠一人格 + 记忆重塑，P01–P04 已实施）（执行记录 [同名](docs/execution/settings-consolidation.md)）——设置项收束 + 设置页卡片化 + 空闲问候；macOS 同构部分仍待 Mac 验证。功能表 [FEATURE_PARITY.md](docs/FEATURE_PARITY.md) 只汇总状态，不覆盖计划。
- 每个对话先确认角色与用户授权，读取 AGENTS、指定计划及执行记录，再用 `git status`、`git log`、`git diff` 和未跟踪文件核对基线。已有用户改动必须保留。
- **规划**：只读调查，明确目标/非目标、逐文件增改删、约束、REQ 编号、依赖、验收矩阵、命令和完成条件。用户要求“不修改文件”时只在对话输出；授权落盘后才写指定文档，不写产品代码。
- **执行**：先复述关键目标和验收标准，再按指定计划实施。可作计划内的局部实现选择，不得自行缩减功能、将完整交付改成骨架、跳过验收或改变架构边界。
- **暂停**：基线重叠冲突、目标互斥、必须改变契约/依赖/数据格式/平台范围，或无法满足必要验收时，说明文件证据、影响、建议和所需决定，暂停受影响工作。若用户要求整体暂停则整体暂停；否则可继续不依赖冲突的已授权工作。普通编译错误与计划内修复不要求重新批准。
- **计划变更**：执行者只能在执行记录中提出偏差/变更请求，不能修改目标或验收来使现状“合规”。计划修订须记录用户确认、版本及改变的 REQ；不能凭空写“已批准”。
- **证据**：每个 REQ 分开记录实现、自动测试、人工验收状态；命令须记录目标程序、架构、退出码、执行时工作区和结果位置。保留失败及 SKIP；旧入口测试、编译成功和执行者声明不能替代新入口验收。
- **审查**：默认只读，对照契约、实际 diff（含未跟踪文件）和证据，按 REQ 报告优先级、位置、影响、缺少测试。不因为发现问题就重新实现；只读审查不自行更新文档。
- **完成**：所有本次范围内必需项通过、有证据、无未解决审查项才可宣布完成。必需测试受阻/跳过、人工检查未做时写“未完成/待验收”，不自行降低标准。
- 新测试必须隔离数据目录、网络端口、自启项和凭据；应用宿主测试也必须在入口初始化前隔离。不得让测试启动默认用户实例。

## 当前状态（2026-10-04）

- Git HEAD 与工作区以只读查询为准；当前四阶段工作基于 `07400ce`，大量实现仍未提交。不得覆盖或还原已有改动。
- 最终架构为共享 Rust core/runtime/ABI3 + macOS SwiftUI/AppKit + Windows C#/WinUI 3/Win32；旧 egui 与 shell 已于 2026-09-22 删除，历史可从 Git 查阅。
- macOS 最低版本为 26，统一由 `apps/macos/project.yml` 管理；XcodeGen 生成工程后同步记录工具版本，并验证包内 `LSMinimumSystemVersion`。
- 本地宠物单击只选择、双击才切换，Codex 候选双击导入；设置使用系统原生控件，打开时显示 Dock 图标，关闭后恢复菜单栏模式。
- macOS 桌面陪伴主线见 `docs/plans/macos-companion-evolution.md` 与同名执行记录。聊天、历史、记忆、人格来源已落地；完整 Codex 桌面一致性、macOS 26 实机、Windows 共享回归仍待验收，自动门禁通过不能关闭这些项。
- 2026-10-04 用户要求修复注视、对齐浮层 UI、减少高级设置并清理旧代码/参考文件/缓存。macOS 设置只暴露模型连接、人格、记忆与历史控制，以及必要桌面行为；隐藏调参的已有配置保留，界面仅提交可见字段，客户端合并完整投影和未确认修改再发送。
- 自动 XCTest/smoke 使用隔离 home、端口、自启目录与假 API Key 环境变量，入口初始化前完成隔离，避免正式 Keychain 授权弹窗。真实 Keychain 与登录自启只在专门人工验收时测试。
- 已清理旧 `docs/archive/`、无入口的 Windows `.rc`、旧构建副本及一次性光标/启动计时脚本；macOS 构建说明集中到 README，验收步骤集中到平台清单。当前计划、执行证据、测试夹具和用户数据保留。
- Windows 原生已完成早期人工 W 矩阵；新增 W33、发布/登录/干净机器项仍以 `docs/WINDOWS_VERIFICATION.md` 和执行记录为准，macOS 证据不能替代 Windows 证据。

## 架构约定

- 一个仓库、一个 workspace、共享 `petsona-core` / `petsona-runtime` / `petsona-ffi`；Windows 与
  macOS 各自拥有原生前端，可独立发布（tag `windows-v*` / `macos-v*`），不长期维护平台分支。
- 最终前端不依赖 egui、eframe 或 winit。**旧 egui UI 与两个旧 shell 已于 2026-09-22 删除**（REQ-W15）；
  需要历史对照时用只读 `git show` 查询，不恢复死代码。
- Rust 负责业务状态、动画与平台无关几何；原生前端负责 UI 主线程、窗口、输入、托盘、菜单和渲染。
- `contracts/petsona.h` 是跨语言边界；不跨边界传递 Rust 引用、容器或分配器所有权。
- `PhysicalRect` 是跨混合 DPI 的唯一几何单位（物理像素）；逻辑点只在平台前端边界换算。
- Windows 目标使用 WinUI 3 + Win32，macOS 使用 SwiftUI + AppKit；平台差异通过各自原生服务实现。
- 接口清单与迁移历史见 `docs/PLATFORM_ARCHITECTURE.md` 和 `docs/plans/native-ui-rewrite.md`。

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
- 状态协议：`127.0.0.1:17872`，`POST /state`、`GET /health`、`GET /pets`；`ttlMs: 0` 表示不过期；
  `{"source":"x","action":"clear"}`（无需 `state`）解除该 source 自己的覆盖 —— 粘滞状态的协议侧出口（CR-W2 选项 A，smoke N25）。
- 宠物帧数按图集实际绘制推断（发行版 V2 的 idle 实际画 7 帧，官方表写 6）；row9 = 右侧方向
  姿势表、row10 = 左侧方向姿势表，每行中间帧是中性姿势；依据与测量方法见 `pet/state.rs` 和
  `pet_inspect`。
- 日志：数据目录 `logs/petsona.log`；单实例锁：数据目录 `petsona.lock`。
- 启动耗时基线（2026-09-21，Release，隔离 home，同一构建）：`Start-Process` → 宠物窗可见
  本地热启动约 0.55 s、本地冷启动约 3.2 s、从 `\\wsl.localhost` UNC 路径启动约 4.2 s；
  慢在 WinUI/WindowsAppSDK 宿主与托管依赖初始化，不在宠物窗创建；历史证据见 `docs/execution/windows-native-rewrite.md` E-W15d。
- 当前 release profile：`lto = "thin"`、`codegen-units = 1`、`strip = true`、`panic = "unwind"`。
  FFI 的 panic 隔离以具体测试和人工验收记录为准，不能仅因配置为 unwind 就宣布通过。
- Windows 打包产物：`dist\Petsona-windows-x64-<version>.zip`（含 exe、图标、VERSION、README）；
  exe 图标由 `apps/windows/Petsona/Petsona.csproj` 的 `<ApplicationIcon>` 直接内嵌
  `packaging\windows\Petsona.ico`（旧 Rust build.rs 已随旧外壳删除）；窗口/任务栏图标由
  `Petsona.Native.WindowIcon` 从 exe 提取后 `WM_SETICON` 给各窗口。

## 坑与教训（别再重新推导）

- **拥有窗口的线程必须抽消息**：跨线程 `GetWindowTextW` 是同步 `SendMessage`，会永久阻塞。
  原生窗口查询必须确认所属线程和消息循环，避免跨线程同步消息死锁。
- **Win11 会给顶层窗口画系统边框 / 圆角**：剥掉所有经典 frame 样式后仍有一条 1px 边框（透明子窗上就是顶部白线）。
  修法：`DWMWA_BORDER_COLOR = DWMWA_COLOR_NONE` + `DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_DONOTROUND`，
  宠物 / 气泡 / 影子 / 输入框都要设；旧系统会拒绝属性，失败无害。
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
- **不要每帧重复发 `InnerSize` / `WindowLevel`**：会反复 `SetWindowPos` 导致闪；
  用 `applied_window_size` / `applied_always_on_top` 缓存。
- **窗口类不设光标 = 光标形状被"冻住"**：`WNDCLASSEXW.HCursor` 为 NULL 且窗口过程不处理 `WM_SETCURSOR` 时，Windows 不安装光标，
  窗口上会保留进入前的形状 —— shell 启动程序时设的 `IDC_APPSTARTING`（忙碌圈）就卡在宠物上，直到鼠标移进别的有光标的窗口。
  修法：注册窗口类时设 `HCursor = LoadCursor(NULL, IDC_ARROW)`，并在 `WM_SETCURSOR` + `HTCLIENT` 时 `SetCursor` 后返回 1；托盘 owner 窗口同样补。
  验证受限：窗口类是**进程局部**的，`GetClassInfoEx` 跨进程必失败；`SetCursor` 也只对拥有窗口的线程生效，跨进程无法伪造形状。跨进程判据用
  "发 `WM_SETCURSOR` 看返回值"（1=接管），并现场注册一个"NULL 类光标 + DefWindowProc"的对照窗口返回 0 自校准（smoke N23/N24）。
- **协议状态由 source 拥有生命周期**：`POST /state` 的 `source` 在内部变成 `hook:<source>`；**同一 source 必然覆盖自己**（`running → review → idle` 都能发），不同 source 才比优先级（failed 90 > waiting 80 > running 70 > review 60 > waving/jumping 40 > look 行 20 > running-left/right 10 > idle 0，**优先级相同也拒绝，后来者输**）。因此 `ttlMs:0` 的粘滞状态只能被「同 source 的下一条」或「更高优先级的其他 source」顶掉：用户点击（`native` 源 `waving` 40）、拖动（10）和 Composer 聊天（只发对话、不推状态）都顶不掉——这是既定设计（用户交互不打断 agent 状态）。`StateEvent.action` 现已生效：`"clear"`（trim + 大小写不敏感）解除**该 source 自己**的覆盖，body 里同时带 `state` 时 clear 优先，未知 action 不改变原逻辑；对应 Windows smoke N25。
- **WinUI XAML 事件可能在 `InitializeComponent()` 期间触发**：`Slider` 设置 `Minimum/Maximum` 就会 `ValueChanged`，此时后面声明的控件还是 null → 处理函数抛 `NullReferenceException`，**窗口对象仍在但永远不显示**（异常被上层吞掉，进程存活，表现为「设置窗打不开」而日志里什么都没有）。修法：处理函数先 `if (_suppressEvents || 目标控件 is null) return;`；回归点是 smoke N12/N19（空库首启 + 设置窗聚焦）。
- **不激活窗口很重要**：宠物 / 气泡 / 菜单都不抢焦点（Windows `WS_EX_NOACTIVATE`，
  macOS 非激活面板），否则会打断用户正在编辑的应用。
- **状态协议偶发空响应的根因**：Windows `accept()` 的 socket 继承监听 socket 的非阻塞模式；
  accept 后显式 `set_nonblocking(false)`（`every_request_gets_a_response` 回归测试）。
- **HTTRANSPARENT 只在同线程窗口可靠转发**：跨进程需要 `WS_EX_TRANSPARENT`，但会引入抖动——
  “像素级穿透”和“零闪烁”要权衡。
- **WSL 起来的 PowerShell 会污染 `PATHEXT`**（本机实测只剩 `.CPL`）：`Get-Command dotnet` / `rustc` 找不到，`& "C:\...\dotnet.exe"` 也会**静默失败**
  （无输出、无退出码）。`verify-windows.ps1` / `package-windows.ps1` 已在开头把它恢复成 Windows 默认值，新脚本照抄那段防护。
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

- **提交总结是交付的一部分**：每轮修改文件后（无论用户是否准备立刻提交），交付说明中必须给出
  未提交改动的分组摘要与可直接执行的 `git add` / `git commit` 命令；AI 不执行提交，由用户随时手动提交。

# windows-native-rewrite：执行与审查记录

## 接手

- 计划路径/版本：[docs/plans/windows-native-rewrite.md](../plans/windows-native-rewrite.md) v1.1（2026-09-20 对齐 macOS 暂缓清单后）。
- 用户执行授权 / 日期：2026-09-20 用户确认并行推进 Windows 线、冻结旧 Windows 线复测、授权"批次 0 预研 → 落盘计划"路线。产品代码实施按本计划 B0 起由后续执行对话推进。
- HEAD、分支、dirty：`c4fc413` / `main`；工作区干净（2026-09-20 核对）。预研探针只写 `.scratch/winui-probe`（gitignored）。
- 关键目标与验收复述：以 C# / WinUI 3 + Win32 实现 Windows 原生前端，消费同一 `contracts/petsona.h`（ABI 3）与 runtime worker；REQ-W01～W14 全部有实现/自动/人工证据后接管 Windows 产品入口；REQ-W15 清理须与 macOS 线联动。
- 本次实际状态：**B0～B5 已落地（B5：脚本/打包/验收文档）；`verify-windows.ps1 -Full` 端到端全绿（cargo 90 + dotnet 21 + native smoke 13 + 打包结构），包已通过干净目录运行验证。**

## 范围对齐（v1.1，2026-09-20）

用户要求 Windows 线对齐 macOS 开发中明确暂缓的功能清单，依据 `docs/execution/native-ui-rewrite.md` 8.9 节。
计划已修订为 v1.1，新增 §3.1 暂缓表并收缩 REQ-W04/W06/W07/W10 与 B4：

- 暂缓：自动活动提醒、重力、native 状态协议设置界面、宠物图标托盘化、影子→编辑按钮动画、透明度设置、多屏/Retina/Spaces/工作区适配。
- 保留：单屏位置持久化（物理像素）、像素穿透、注视/caret、缩放档位、气泡/Composer、宠物库、人格/记忆/DeepSeek、菜单「立即活动」基础接线。

### 跨线观察（供 macOS 线复核，不在本线修复）

- macOS `PetView.alphaAt` 的命中回退使用的是 atlas **最后一行**（`atlas.pixelsHigh - cellHeight`）；core 的权威定义中 `PetState::Idle` 是 **row 0**（`crates/petsona-core/src/pet/state.rs`）。Windows 线按 row 0 实现 idle 并集回退；建议 macOS 线复核该行为是否为缺陷。

## 要求与文件

| REQ | 改动文件与具体行为 | 实现状态 | 自动验证 | 人工验收 | 剩余工作 |
|---|---|---|---|---|---|
| REQ-W01 | `apps/windows` 工程/锁文件/CI 已写入（21 个文件，含 3 份 packages.lock.json） | 部分（CI 未运行过） | 通过：restore --locked-mode、build Debug+Release 0 警告、format 无警告（E-W1b～e） | 不适用 | CI 首次运行验证 |
| REQ-W02 | `Petsona.Core` LibraryImport 绑定 + EngineClient + 线程约束 | 部分（缺 panic 注入与随包复制） | 通过：7/7（AbiTests 4 + EngineClientTests 3，E-W1f） | 不适用 | panic 注入（后续批次）、DLL 随包（B5） |
| REQ-W03 | Win32 分层窗（无边框/置顶/不激活）+ GDI+ 帧渲染（UpdateLayeredWindow）+ 像素穿透（idle row 0 并集回退） | 已实现 | 通过：端到端运行 + 帧导出 + 截图确认（E-W2d） | **2026-09-21 人工通过**：W1 窗口视觉 + W4 穿透落点 | — |
| REQ-W04 | 单双击（320ms）/拖动（4px 阈值）区分、位置物理像素保存、托盘图标 + 原生菜单（专用 STA 线程）、不抢前台 | 已实现 | 通过：N15 点击、N17 托盘菜单、N20/N22 拖动动画（E-W9a/E-W14d） | **2026-09-21 人工通过**：W2 托盘菜单（含溢出面板 / 固定到任务栏）+ W3 点击、双击、拖动 | — |
| REQ-W05 | 空库自动开设置；导入文件夹/zip、Codex 扫描与双击导入、切换、导出、删除、重复 ID 覆盖确认（内联确认面板） | 已实现 | 命令层经 SettingsFlowTests 覆盖；N12 空库首启；导入 / 导出链路仍无端到端自动测试 | **2026-09-21 人工通过**：W7 导入（文件夹 / zip / Codex）、导出、删除、覆盖确认 | — |
| REQ-W06 | 设置页全量：缩放档位/穿透、DeepSeek 全配置 + 凭据保存/清除、人格 CRUD/复制/导入导出/22 字段编辑、记忆配置与事实管理、HKCU 自启 | 已实现 | 通过：SettingsFlowTests + SystemServiceTests（13/13）+ UI 渲染截图（E-W3a～c） | **2026-09-21 人工通过**：W7 缩放 / 穿透 / DeepSeek / 凭据保存 / 人格 / 记忆 / HKCU 自启开关（登录后实际自启见 W 表 D5） | 凭据存在性显示（后续微调） |
| REQ-W07 | 气泡（宠物上方 10px，协议驱动）、编辑按钮（下方 16px，点击打开 Composer）、Composer（380×190、位置翻转/夹取、Enter 发送/Shift+Enter 换行/Esc 关闭、草稿保留、IME 交给平台控件） | 已实现 | 通过：端到端渲染 + 按钮点击打开路径（E-W4b/c）+ N18 聚焦 | **2026-09-21 人工通过**：W6 Enter 发送 / Shift+Enter / Esc 草稿 / 中文 IME + W13 面板聚焦 | — |
| REQ-W08 | 协议经新入口验证；HKCU 自启（含隔离值名测试）；单实例/日志由 runtime 提供 | 已实现 | 通过：N7 协议状态 / N8 TTL 回退 / N10-N11 单实例 / N13 端口释放（E-W6a）+ 自启测试（13/13） | **2026-09-21 人工通过**：W7 自启开关 + W10 退出释放端口 + W11 状态协议 5 条步骤 | 无（粘滞状态解除方式记录在 `WINDOWS_VERIFICATION.md` W11 小节；可选改动见 CR-W2） |
| REQ-W09 | 全局光标 33ms 采样（移动阈值 1px）、椭圆进入 40%/退出 50% 迟滞、22% 死区、随宠物移动；Composer 打开时注视输入窗中心 | 已实现 | 通过：GazeFilterTests / GazeStabilizerTests + N21 注视跨行（E-W14c） | **2026-09-21 人工通过**：W5 全方向跟随、范围、静止保持、死区与迟滞观感 | 字符级 caret 精度（当前为窗口中心近似） |
| REQ-W10 | 物理像素位置持久化（单屏）：拖动结束写 `config.json` 的 `startPosition`，重启还原；重力/活动/多屏/工作区夹取按 §3.1 暂缓 | 已实现 | 通过：端到端拖动 → 保存 (1922,904) → 重启还原一致（E-W5b）+ 单测重启持久化 | **2026-09-21 人工通过**：W3 拖动手感与松手保存 | 工作区夹取解冻后补 |
| REQ-W11 | faulted 分支（本地错误气泡 + 停注视 + 1s 心跳）、Dispose 顺序、销毁拒绝调用、线程约束、退出释放端口/锁 | 已实现 | 通过：EngineLifecycleTests 4 项（21/21 总计，E-W5a/d）+ N13 端口释放 | **2026-09-21 人工通过**：W10 托盘菜单退出、进程结束、无残留窗口 / 托盘 | FFI panic 注入（无公开接口，两端共有） |
| REQ-W12 | 测试隔离：每测试独立 home + `stateServer.enabled=false` | 部分（探针未建） | 通过：隔离逻辑在 7/7 内验证 | 不适用 | 探针与生产包检查（B1 起） |
| REQ-W13 | `verify-windows.ps1` 切到新入口（含 UNC/PATHEXT/工具定位适配）、`windows-smoke.ps1` 重写为 24 项 native smoke、`package-windows.ps1` 改为 dotnet publish + 正斜杠 zip、release workflow 加 setup-dotnet | 已实现 | 通过：`verify-windows.ps1 -Full` 多轮全绿（最近 E-W15e：24/24）；干净目录解压运行 /health 通过（E-W6a/d） | **2026-09-21 人工通过**：W1 打包版启动 + W10 退出 | D6 干净机器（无 WindowsAppRuntime）待人工 |
| REQ-W14 | `WINDOWS_VERIFICATION.md` 新增「W. 原生前端验收」12 项人工矩阵；`WINDOWS_ISSUES.md` 顶部冻结声明 | 已实现 | 文档审查通过（E-W6f） | **2026-09-21 人工通过**：W1～W11、W13、W14（首轮～第十轮实机复测）；W12 暂缓 | 无（W12 解冻需用户确认） |
| REQ-W15 | 旧线清理 | 未开始（收尾） | 未跑 | 不适用 | 两端联动确认 |

## 命令证据（追加，不覆盖失败历史）

批次 0 预研（2026-09-20，机器 `DESKTOP-UI2ET48`，Windows 11 25H2 build 26200；源码 `\\wsl.localhost\archlinux\home\cwwwwy\Petsona`，探针复制到 `C:\Users\happyddz\AppData\Local\Temp\petsona-winui-probe`）：

| 证据ID/时间 | REQ与代码版本 | cwd/OS/架构/目标入口 | 完整命令 | 退出码/测试数 | 结果路径及限制 |
|---|---|---|---|---|---|
| E-W0a | 批次 0 工具链验证（探针，非产品代码） | Windows 本地 cwd；`cmd.exe`；.NET SDK 10.0.401 | `dotnet build -c Debug`（net10.0-windows10.0.26100.0 + WindowsAppSDK 2.5.1，unpackaged） | exit 0；0 警告 0 错误 | 产物 `bin\Debug\net10.0-windows10.0.26100.0\win-x64\Probe.dll`；首次 restore 32.9s（联网 NuGet） |
| E-W0b | 同上，运行验证 | Windows 桌面会话 | `Start-Process Probe.exe`；6s 后检查并 `Stop-Process` | 进程 RUNNING → STOPPED | unpackaged 应用可启动；窗口视觉不构成产品验收 |
| E-W0c | 批次 0 工作流验证 | Windows 本地 cwd 构建 UNC 源码 | `dotnet build \\wsl.localhost\archlinux\home\cwwwwy\Petsona\.scratch\winui-probe -c Debug` | exit 0；0 警告 0 错误 | 12.6s 增量；证明不需要 Windows 侧源码副本 |
| E-W0d | CI 门禁可行性 | Windows 本地路径 | `dotnet restore --use-lock-file`；`dotnet restore --locked-mode` | 两次 exit 0 | 生成 `packages.lock.json`；WindowsAppSDK 2.5.1 依赖树已锁定 |
| E-W0e | 运行通道限制 | Windows | `cmd.exe` / `pwsh.exe` 调用 dotnet | — | `cmd.exe` 正常但不可以 UNC 为 cwd（需 `cd /d` 本地后绝对路径）；Codex pwsh 通道 native 输出异常（工具通道问题，真实脚本行为由 B0 复核） |

环境清单（只读核对）：.NET SDK 10.0.401；Windows SDK 10.0.26100；Windows App Runtime 1.7/1.8/2.4.0/2.5.1 已装；**无 Visual Studio**；Windows 侧 rustup 有 1.98.0-msvc 与 stable-gnu/msvc，无 MSVC `link.exe`（沿用 GNU 回退）。

### B0 实施证据（2026-09-20）

| 证据ID/时间 | REQ与代码版本 | cwd/OS/架构/目标入口 | 完整命令 | 退出码/测试数 | 结果路径及限制 |
|---|---|---|---|---|---|
| E-W1a | REQ-W02 / T-WR2 | WSL Arch x86_64 交叉编译 Windows 目标 | `cargo build -p petsona-ffi --release --target x86_64-pc-windows-gnu` | exit 0；44.2s | 产出 `target/x86_64-pc-windows-gnu/release/petsona_ffi.dll`（PE32+ x86-64，9.6 MB）；WSL 自带 mingw-w64，不需要 Windows 侧 cargo |
| E-W1b | REQ-W01 / T-WB | Windows cmd；源码经 UNC | `dotnet restore apps/windows/Petsona.sln --locked-mode` | exit 0 | 首次 restore 生成 3 份 `packages.lock.json`；locked 复核通过 |
| E-W1c | REQ-W01 | 同上 | `dotnet build apps/windows/Petsona.sln -c Debug --no-restore` | exit 0；0 警告 | Debug 构建是 `dotnet format` 工作区评估的前置（已写入 CI 注释） |
| E-W1d | REQ-W01 | 同上 | `dotnet format apps/windows/Petsona.sln --verify-no-changes --no-restore` | exit 0；0/26 文件需改 | 修复默认 `Platform=x64` 后无工作区警告（此前 Debug 引用解析失败） |
| E-W1e | REQ-W01（B0 骨架） | 同上 | `dotnet build apps/windows/Petsona.sln -c Release -p:Platform=x64 --no-restore` | exit 0；0 警告 0 错误 | WinUI（net10.0-windows10.0.26100.0 + WindowsAppSDK 2.5.1）+ Petsona.Core + Petsona.Tests |
| E-W1f | REQ-W02 / T-WT | 同上 | `dotnet test apps/windows/Petsona.sln -c Release -p:Platform=x64 --no-build` | exit 0；**7/7 通过** | 真实加载 `petsona_ffi.dll`：ABI 布局/序数 4 项 + 引擎隔离/命令往返（scale+bubble）/无效命令 3 项 |
| E-W1g | T-WR0（部分） | WSL | `cargo fmt --all -- --check`；`cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0 | 本轮未改 Rust 源码；lint 门禁保持绿 |
| E-W1h | T-WR0（部分） | WSL（提权） | `cargo test -p petsona-core -p petsona-runtime -p petsona-ffi --locked` | exit 0；56+3+4=**63 passed** | 沙箱禁止回环绑定，4 个协议测试需提权；`cargo test --workspace` 在 WSL 因缺 `-lxdo`/`-lgtk-3` 系统库失败（旧 egui 外壳的 Linux 依赖，与本次改动无关），完整 workspace 由 CI 两端覆盖 |

### B1 实施证据（2026-09-20）

| 证据ID/时间 | REQ与代码版本 | cwd/OS/架构/目标入口 | 完整命令/步骤 | 退出码/测试数 | 结果路径及限制 |
|---|---|---|---|---|---|
| E-W2a | REQ-W03/W04 构建 | Windows cmd；UNC 源码 | `dotnet build apps/windows/Petsona.sln -c Debug --no-restore` | exit 0；0 警告 | B1 源文件：PetWindow / NativeWin32 / SpriteRenderer / FrameScheduler / TrayService / AppController / SettingsWindow |
| E-W2b | REQ-W02/W03 测试 | 同上 | `dotnet test … -c Debug --no-build` | exit 0；**11/11** | AbiTests 4 + EngineClientTests 3 + SpriteFrameTests 4（行列、源矩形、局部坐标、clamp） |
| E-W2c | REQ-W03/W04/W08 端到端 | 隔离 `PETSONA_HOME=%TEMP%\petsona-b1`（自绘 testdata 宠物，端口 17899） | 启动 `Petsona.exe` → 探针检查 → 截图 → 停止 | PROC_ALIVE=True；HEALTH ok（pet=TestPet, state=idle）；WINDOW visible pos=1688,784 size=192x208；STOPPED=True | 截图为屏幕窗口区域；宠物精灵正确显示在屏内 |
| E-W2d | REQ-W01/W03 门禁 | Windows cmd | `dotnet restore --locked-mode` + `dotnet format --verify-no-changes --no-restore` + `dotnet build -c Release -p:Platform=x64` + `dotnet test -c Release --no-build` | exit 0；Release 0 警告；**11/11** | 完整 B1 门禁链 |
| E-W2e | 缺陷修复记录 | — | 初始定位 bug：窗口落在屏幕外（pos 1878,991 超出 1920x1080） | — | 根因：`ApplyInitialPosition` 早于窗口尺寸应用（用 1x1 计算右下角偏移）。修复：先 `_window.Update` 应用尺寸，再定位（E-W2c 复验 1688,784） |

### B2 实施证据（2026-09-20）

| 证据ID/时间 | REQ与代码版本 | cwd/OS/架构/目标入口 | 完整命令/步骤 | 退出码/测试数 | 结果路径及限制 |
|---|---|---|---|---|---|
| E-W3a | REQ-W05/W06/W08 测试 | Windows cmd；UNC 源码 | `dotnet build … -c Debug --no-restore`；`dotnet test … -c Debug --no-build` | exit 0；0 警告；**13/13** | 新增 SystemServiceTests（HKCU Run 隔离值名写删）与 SettingsFlowTests（DeepSeek/记忆/人格/事实往返）；`Petsona.Core`/`Petsona.Tests` TFM 改为 `net10.0-windows`（Registry API 平台标注） |
| E-W3b | REQ-W05/W06 运行验证 | 空库隔离 `PETSONA_HOME=%TEMP%\petsona-b2`（端口 17899） | 空库启动 → 等待自动打开设置 → 探针检查 + 截图 → 停止 | PROC_ALIVE=True；SETTINGS_WINDOW visible 1440x753；HEALTH ok（pets=[]）；STOPPED | 截图确认宠物库/外观/DeepSeek 分区与控件渲染正常；人格/记忆/启动区在同一窗口下方 |
| E-W3c | REQ-W01 门禁 | Windows cmd | `dotnet restore --locked-mode` + `format --verify-no-changes` + `build -c Release -p:Platform=x64` + `test -c Release --no-build` | exit 0；Release 0 警告；13/13 | 完整 B2 门禁链 |

### B3 实施证据（2026-09-20）

| 证据ID/时间 | REQ与代码版本 | cwd/OS/架构/目标入口 | 完整命令/步骤 | 退出码/测试数 | 结果路径及限制 |
|---|---|---|---|---|---|
| E-W4a | REQ-W07/W09 测试 | Windows cmd；UNC 源码 | `dotnet build … -c Debug --no-restore`；`dotnet test …` | exit 0；0 警告；**17/17** | 新增 GazeFilterTests（死区、迟滞、椭圆内外）；PetWindow 改用共享 LayeredPresenter |
| E-W4b | REQ-W07 端到端（气泡/按钮） | 隔离 home + 端口 17899 | 启动 → POST waiting+message → 枚举窗口 + 截图 | PROC_ALIVE=True；OVERLAY 174x49 @ 宠物上方 10px；OVERLAY 40x40 @ 宠物下方 16px；STATE=waiting；STOPPED | 截图确认气泡文本与圆形编辑按钮渲染正确 |
| E-W4c | REQ-W07 端到端（Composer） | 同上 | Simulate 点击编辑按钮 → 等待 → 截图 | COMPOSER visible 380x190 pos=1540,570（下方空间不足自动翻转到宠物上方并夹取工作区）；截图确认输入框/发送按钮 | 键盘输入发送与 IME 留待 M-W |
| E-W4d | REQ-W01 门禁 | Windows cmd | `restore --locked-mode` + `format --verify-no-changes` + `build -c Release -p:Platform=x64` + `test -c Release --no-build` | exit 0；0 警告；17/17 | 完整 B3 门禁链 |
| E-W4e | 缺陷修复 | — | Composer 首版使用 WinUI 默认 1440x753 且可能越出屏幕 | — | 修复：构造时 `AppWindow.Resize(380x190)`；定位在下方空间不足时翻转到宠物上方并夹取工作区（E-W4c 复验） |

### B4 实施证据（2026-09-20）

| 证据ID/时间 | REQ与代码版本 | cwd/OS/架构/目标入口 | 完整命令/步骤 | 退出码/测试数 | 结果路径及限制 |
|---|---|---|---|---|---|
| E-W5a | REQ-W10/W11 测试 | Windows cmd；UNC 源码 | `dotnet build … -c Debug --no-restore`；`dotnet test …` | exit 0；0 警告；**21/21** | 新增 EngineLifecycleTests：Dispose 后拒绝调用且幂等、跨线程调用拒绝、销毁释放状态服务端口与实例锁、位置重启持久化 |
| E-W5b | REQ-W10 端到端（拖动） | 隔离 home + 端口 17899；真实 SendInput | 拖动宠物 → 读 config → 重启 → 读窗口位置 | PART1 1778,832 → **1922,904**；config `startPosition` = **1922,904**；PART2 重启还原 = **1922,904** | 位置为物理像素，保存与还原精确一致；拖动后窗口可超出屏幕（工作区夹取属 §3.1 暂缓） |
| E-W5c | REQ-W04 端到端（托盘） | 同上 | PostMessage 请求菜单 → 截图 → Esc | 菜单窗口弹出、截图确认 5 项菜单（打开设置/更换宠物/隐藏显示/立即活动/退出）；Esc 关闭后进程存活 | 菜单"退出"项的自动交互未复现（键盘导航），留 M-W |
| E-W5d | REQ-W01 门禁 | Windows cmd | `restore --locked-mode` + `format --verify-no-changes` + `build -c Release -p:Platform=x64` + `test -c Release --no-build` | exit 0；0 警告；21/21 | 完整 B4 门禁链 |
| E-W5e | 脚本诊断记录 | — | 拖动模拟首版等待 150ms 时点击被穿透吞掉 | — | 诊断确认拖动逻辑正确（窗口随光标 delta 移动）；模拟需等穿透状态切换，等待改为 ≥400ms 后通过 |

### B5 实施证据（2026-09-20）

| 证据ID/时间 | REQ与代码版本 | cwd/OS/架构/目标入口 | 完整命令/步骤 | 退出码/测试数 | 结果路径及限制 |
|---|---|---|---|---|---|
| E-W6a | REQ-W13/W14 全链路 | PowerShell 5.1，仓库经 UNC | `powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1 -Full` | exit 0；cargo 90 项（app25/core56/ffi3/runtime4/shell-windows2）；dotnet **21/21**；**native smoke 13/13**；打包结构检查通过 | 两次完整运行全绿；首次冷编译约 6 分钟，热缓存约 1.5 分钟 |
| E-W6b | 脚本适配修复集 | — | UNC 开发流 / PS 5.1 兼容 | — | ① UNC 根不支持为当前目录 → `New-PSDrive` 映射；② WSL interop 破坏 `PATHEXT`（=.CPL）→ 脚本恢复默认；③ Windows cargo 在 UNC 上无法建增量锁 → UNC 时 `CARGO_TARGET_DIR` 切本地 `%TEMP%`、DLL 镜像回仓库 target、`PETSONA_FFI_DLL` 传给测试；④ smoke `$home` 与只读 `$HOME` 冲突 → 改名 `$dataDir`；⑤ PS 5.1 读无 BOM 的中文 `.ps1` 乱码 → 写 UTF-8 BOM（并清理双 BOM）；⑥ 设置窗标题在 XAML 应用前是应用名 → N12 改「窗口类 + PID」匹配；⑦ N2/N5 加窗口轮询 |
| E-W6c | 打包缺陷修复 | — | `package-windows.ps1` | — | ① `-SkipBuild` 原样传 `--no-build` 会产出只有 4 个附加文件的**空壳包** → 改 `--no-restore` + publish 退出码与 `Petsona.exe` 存在性断言；② PS 5.1 的 `ZipFile::CreateFromDirectory` 写反斜杠条目名，跨平台解压会碎成单文件 → 手工 `ZipArchive` 逐条加正斜杠条目（实测 58 条目、0 反斜杠） |
| E-W6d | REQ-W13 干净目录运行 | 解压后的包目录 | 解压 zip（58 文件）→ 隔离 `PETSONA_HOME` + 端口 61111 启动 `Petsona.exe` | alive=True；`/health ok=true` | 解压运行通过；D6「无 WindowsAppRuntime 的干净机器」仍需人工 |
| E-W6e | REQ-W13 CI | — | `.github/workflows/release-windows.yml` | — | 增加 `actions/setup-dotnet`（global-json-file）；fast gates 沿用 `verify-windows.ps1`（不含 GUI smoke，CI 无交互桌面） |
| E-W6f | REQ-W14 文档 | — | `docs/WINDOWS_VERIFICATION.md`、`docs/WINDOWS_ISSUES.md` | — | 新增「W. 原生前端验收」（W1–W12，含暂缓标注）；旧清单加冻结声明并映射到 W 节 |

### B5 验收反馈诊断与修复（2026-09-20，用户首轮人工验收）

用户按方式 2（隔离 `PETSONA_HOME`）验收后反馈三点，诊断与处置如下：

| 反馈 | 诊断结论 | 处置 |
|---|---|---|
| 空库启动却出现"编辑按钮"、没有设置窗口 | 编辑按钮来自**默认数据目录实例**（用户双击 exe 绕过 `PETSONA_HOME`；默认库里有 boba / Superintendent_Petdex）。**真缺陷**：同数据目录的第二个实例在 `InstanceLock::acquire` 失败 → worker `set_fault` → FFI `Snapshot()` 返回 RuntimeFailed → C# `EngineClient.Snapshot()` 抛异常，而故障分支在异常之后**不可达** → 界面空白、无任何错误提示 | 修 `AppController`：捕获 `RuntimeFailed/Panic` 后走 `HandleFault()`（错误气泡 + 停注视 + 1s 心跳）；错误气泡位置夹取到屏幕内；锁冲突映射为中文提示；tick 异常写入 `<PETSONA_HOME>/logs/windows-native-error.log` |
| 托盘右键无反应 | `Shell_NotifyIconGetRect` 返回的是 **Win11 溢出面板箭头**的位置：新注册的托盘图标被系统默认收进"隐藏的图标"。回调处理器经模拟消息（`PostMessage(hwnd, WM_APP+2, 0, WM_LBUTTONUP)`）验证**完全正常**（菜单弹出、Esc 关闭） | 非缺陷；使用说明更新（固定图标显示 / 展开溢出面板）；`TrayService` 增加 `windows-native-tray.log` 诊断 |
| （基线澄清）端口冲突 | `sync_state_server` 绑定失败只记录 status、不 faulted（session.rs），与旧线一致 | 无需修改 |

| 证据ID | 内容 | 结果 |
|---|---|---|
| E-W7a | `dotnet test`（新增 `SecondEngineOnTheSameHomeFaultsInsteadOfSilentlyFailing`） | **22/22 通过** |
| E-W7b | `verify-windows.ps1 -Full`（修复后） | exit 0；smoke **13/13**；打包结构检查通过 |
| E-W7c | 同数据目录第二实例 UI 验证 | 显示错误气泡（245x143，位置已夹取），不再空白 |
| E-W7d | 托盘回调模拟验证 | 模拟 `WM_APP+2/LBUTTONUP` → 菜单窗口出现；真实点击落在溢出箭头（OS 行为） |

### B5 人工验收第二轮反馈与修复（2026-09-20）

用户完整验收 11 项后的处置（对应验收卡编号）：

| 反馈 | 根因 | 修复 |
|---|---|---|
| 1/3/4/5 宠物不出现（编辑按钮/气泡正常） | **用户真实宠物图集是 webp**（boba/rocky），而 System.Drawing (GDI+) 不能解码 webp → `GetFrame` 返回 null → 精灵窗口不显示，而编辑按钮/气泡不依赖帧数据 | `SpriteRenderer` 增加 **WinRT/WIC 解码回退**（`BitmapDecoder.GetPixelDataAsync`，GDI+ 失败时异步解码并缓存）；新增 **N14 webp 图层 smoke**（自绘 webp 夹具 `testdata/v2-test-pet-webp`）；定位改为等真实尺寸（异步解码首帧 1x1 会让默认右下角定位出屏）；编辑按钮改为仅在宠物窗口实际可见时显示 |
| 2/10 托盘右键无反应 | `Shell_NotifyIconGetRect` 返回的正是 Win11 **溢出面板箭头**位置：新图标被系统默认收进"隐藏的图标"；回调处理器经 `PostMessage(WM_APP+2/LBUTTONUP)` 模拟验证完全正常 | 非缺陷；文档与指引更新（固定图标 / 溢出面板两条路径），并明确**右键宠物**也能弹同一菜单 |
| 6③ IME 回车触发换行 | 组合期间 Enter 未被识别为 IME 语义 | `ComposerWindow` 跟踪 `TextCompositionStarted/Ended`，组合期间 Enter 交回输入法（不换行、不发送） |
| 7 设置页应为侧边栏布局 | 单页滚动布局不符合预期 | `SettingsWindow` 改为 **NavigationView 侧边栏**（宠物库/外观与交互/DeepSeek/人格/记忆/启动六个分区），窗口 1000x720 |
| 9 第二实例留气泡不退出 | 原实现只在 faulted 时显示气泡 | 锁冲突时显示 3 秒提示后**自动退出**（smoke N10 现记录 `second instance exits exitCode=0`） |
| 11 协议连接被拒 | 应用未在运行或端口不一致（新隔离目录用 17873，旧文档写 17872） | 非缺陷；指引明确"先确认应用运行 + 使用 17873" |

| 证据ID | 内容 | 结果 |
|---|---|---|
| E-W8a | `dotnet test`（含第二实例 faulted、位置持久化等） | **22/22 通过** |
| E-W8b | `windows-smoke.ps1`（新增 N14 webp） | **13 PASS + 1 SKIP（N9 外部鼠标干扰）+ 0 FAIL** |
| E-W8c | `verify-windows.ps1 -Full`（含解压包冒烟） | exit 0；gates passed |
| E-W8d | webp 渲染验证 | 用户宠物 boba（webp）截图确认渲染与右下角定位正确 |

### B5 人工验收第三轮反馈与修复（2026-09-20）

| 反馈 | 根因 | 修复 |
|---|---|---|
| 1 宠物无交互 | 穿透释放只靠 50ms 的 `WM_TIMER` 轮询，渲染负载下被延迟 → 点击全部穿透到桌面 | 穿透检查**并入每帧 `Update`**（16ms 级）；新增 **N15 真实点击测试**（点击宠物 → `/health` 出现 `waving`） |
| 2 托盘右键无反应（已固定图标） | 未显式设置 `NIM_SETVERSION`，shell 的回调 `lParam` 布局与解析假设不一致 | `NIM_ADD` 后显式 `NIM_SETVERSION(4)`；回调解析同时接受版本 4（事件在高低字）与旧版布局 |
| 3 IME 回车仍换行 | 组合期间 Enter 未标记 `Handled`，TextBox 默认行为插入了换行 | 组合期间 `e.Handled = true`（IME 已在更早阶段确认候选） |
| 4/5 | 侧边栏与第二实例自动退出 | 保持通过 |
| 开发建议 1：宠物库/Codex 候选需要预览 | `pets` 投影缺少 spritesheet 字段（codex 候选已有） | runtime `pets` 投影补 `spritesheet`/`cellWidth`/`cellHeight`（**共享层变更，macOS 待回归**）；设置页列表改为缩略图行（WinRT 全图解码 + 手动裁剪首帧 + `WriteableBitmap`，规避 `BitmapTransform` 对 webp 产出花屏的问题） |
| 开发建议 2：关设置窗/输入框会退出进程 | WinUI 在**最后一个 `Window` 关闭时退出**，而宠物窗是原生 Win32 窗口不计数 | 增加**隐藏生命周期锚点窗口**，关闭面板不再结束进程 |
| 附加缺陷 | `CopyPetsonaFfi` 的两个复制分支都执行，过期的 MSVC DLL 覆盖新的 GNU DLL（settings 读到旧投影） | 改为**优先级单一来源复制**（GNU 优先、MSVC 回退），并删除仓库里过期的 MSVC DLL |

| 证据ID | 内容 | 结果 |
|---|---|---|
| E-W9a | `windows-smoke.ps1`（新增 N15 点击、N14 webp） | **15/15 通过** |
| E-W9b | `verify-windows.ps1 -Full`（含解压包冒烟） | exit 0；gates passed |
| E-W9c | 宠物预览截图 | boba 缩略图正常显示（webp 首帧裁剪） |
| E-W9d | 关窗口不退进程 | 锚点窗口方案（待用户复测确认） |

### B5 人工验收第四轮反馈与修复（2026-09-20）

| 反馈 | 根因 | 修复 |
|---|---|---|
| 1 宠物只有固定一帧（待机/注视/走动都无动作） | **`PetWindow` 只在窗口尺寸变化时重绘**：精灵帧更换（同尺寸）从未上传到分层窗口 | 重绘条件加入**帧对象变化**（`ReferenceEquals(frame, _appliedFrame)`）；新增 **N16 动画测试**（8 次截图采样，distinctFrames>1） |
| 2 托盘固定后右键仍无菜单（图标在溢出面板中） | 图标没有稳定身份，每次启动都被当作新图标收纳 | `NOTIFYICONDATAW.GuidItem` 使用固定 GUID（Windows 可记住"显示在任务栏"的选择）；回调已用 `NIM_SETVERSION(4)` |
| 3 中文输入正常但回车换行（非 IME 场景） | WinUI TextBox 的默认换行发生在 **KeyDown 之前**，原处理器挂得太晚 | 改用 **`PreviewKeyDown`**（隧道阶段）拦截 Enter：无 Shift 时 `Handled=true` 并发送 |
| 建议 1 / 2 | —— | 复测通过（预览、关窗口不退进程） |
| webp 支持比例 | Codex 宠物默认格式为 webp | 确认 webp 为一等格式：smoke 使用 webp 夹具（N14/N16），同时保留 PNG/JPEG 等解码路径以兼容旧包 |

| 证据ID | 内容 | 结果 |
|---|---|---|
| E-W10a | `windows-smoke.ps1`（新增 N16） | **16/16 通过** |
| E-W10b | `verify-windows.ps1 -Full` | exit 0；gates passed |
| E-W10c | 截图采样 | 8 次采样出现 2 种帧哈希交替，确认屏幕内容随动画变化 |

### B5 人工验收第五轮反馈与修复（2026-09-20）

| 反馈 | 根因 | 修复 |
|---|---|---|
| 1（REQ-W04/W07/W09）拖动 / 光标靠近反应慢 | ① Windows 帧调度只按 `snapshot.nextFrameMs` 休眠：idle 280–320 ms、gaze Holding 1 s，没有 macOS 的 16/33 ms 指针轮询下限，命令入队后要等旧 deadline 才重绘。② 拖动每 80 ms 重发同一 `SetState("running-left/right")`，`RuntimeCommand::SetState` 每次重置 `anim_started`，而 running 第 0 帧时长 120 ms，导致动画永远停在第 0 帧。 | ① 删除独立 gaze timer，把 `SampleGaze` 并入帧循环；新增 `FrameCadence`：可见宠物时 `min(nextFrameMs, gazeActive ? 16 : 33)`，无宠物 / 手动隐藏 1000 ms，托盘显示切换 `FrameScheduler.Wake()`；② runtime `SetState` 与协议状态只在可见状态真实变化（或 one-shot 重触发）时重置动画时钟，重复拖动态只刷新 TTL。**共享层变更，macOS 待回归。** |
| 2（REQ-W04）托盘右键无菜单（收纳面板 / 任务栏均无） | 诊断日志显示 `NIM_SETVERSION(4)` 成功但从未出现 callback 日志：Explorer 把通知通过窗口过程直接投递（SendMessage），而 `PetsonaTrayWindow` 的 WndProc 对所有消息 `DefWindowProc`；消息循环的 `WM_APP+2` 分支只覆盖 Posted 消息。 | WndProc 用 `GWLP_USERDATA` 分发 `WM_APP+2`；`TrayCallbackParser` 解析 v4 的 `WM_CONTEXTMENU`（wParam 为屏幕坐标）与 `NIN_SELECT/NIN_KEYSELECT`，兼容旧 low/high word 布局；解析后 PostMessage 回托盘线程再 `TrackPopupMenu`，不在 sent callback 内阻塞 shell。 |
| 3 回车发送 | — | 复测通过，保持 `PreviewKeyDown` 方案。 |
| 4 webp | — | 按一等格式维持（PNG/JPEG 兼容路径保留）。 |
| 5（REQ-W06/W07）输入框 / 设置打开未聚焦 | 宠物窗是 `WS_EX_NOACTIVATE`，新 WinUI 窗口 `Activate()` 后可能没有前台/键盘焦点。 | 新增 `WindowActivation.EnsureForeground`：`SetForegroundWindow + SetFocus` 并在 1.5 s 内每 50 ms 重试；Composer 聚焦 TextBox，Settings 聚焦 NavigationView。 |

| 证据ID | 内容 | 结果 |
|---|---|---|
| E-W11a | runtime 回归测试 `repeated_drag_state_does_not_rewind_the_animation` | 修复后 passing；临时还原修复条件时失败（sprite=8，即第 0 帧），证明测试覆盖根因 |
| E-W11b | `dotnet test -c Debug` | **30/30**；新增 FrameCadence 4 项、TrayCallbackParser 4 项 |
| E-W11c | `verify-windows.ps1 -Full` | exit 0；cargo fmt/clippy/test/release、dotnet **30/30**、**native smoke 19/19**（源码构建与解压包各一轮）、打包结构检查全绿 |
| E-W11d | 隔离诊断（`.scratch/diag17.ps1`） | FAR `WS_EX_TRANSPARENT=True`、NEAR=False、点击后 state=waving；确认快速注视后命中掩码仍含 idle row 0 并集 |
| E-W11e | smoke 取样加固 | N2/N5/N6/N17 改为按本进程 PID 查找窗口；修复首轮门禁中 N9/N15 误取其它 Petsona 实例窗口导致的假失败 |

### B5 人工验收第六轮反馈与修复（2026-09-20）

| 反馈 | 根因 | 修复 |
|---|---|---|
| 1（REQ-W04/W07）拖动时仍固定第一帧，按住不动才播放 running | worker 在拖动态下仍按帧 deadline 推进并发布快照，但拖动时高频 `WM_MOUSEMOVE`（`SetCapture` 后每个鼠标消息都执行 GetCursorPos + SetWindowPos）把 WinUI `DispatcherQueueTimer` 挤住；帧调度 tick 只在鼠标停下后才有机会执行。 | `PetWindow` 新增 `DragMoved` 事件，每次拖动位移后通知 `AppController`；`AppController.OnDragMoved` 以 8 ms 限流直接读取 `Snapshot()` 并调用 `Window.Update()`，不再完全依赖 DispatcherQueueTimer。新增 N20：真实按下拖动 + 14 次窗口哈希采样，`distinctFrames=14`。 |
| 2（REQ-W09）注视立即转头，但全方向不完整 | 跨行注视是分步状态机（先转向 bridge 帧，到达后才切到目标行）；App 只在光标 moved 时发送 `SetGazeTarget`，光标停住后后续步骤永远不发出，姿态停在 bridge / 边缘。 | `SampleGaze` 在光标位于触发椭圆内且不在死区时，每个 poll（16/33 ms）重发目标；引擎在已到位时 `retarget_gaze` no-op。新增 N21：右→`look-row-9`、左→`look-row-10`。该 moved-only 门控 macOS 端同样存在，建议 macOS 线对齐（本轮未改 macOS 前端）。 |
| 3 托盘右键 | — | 复测通过。 |
| 4/5 输入框 / 设置聚焦 | — | 复测通过；smoke N18/N19 在后台自动化会话无法取得前台时改为明确 `[SKIP]`（前台锁），实机 W13 为最终判定。 |
| 6 提交总结 | — | `AGENTS.md` 增加规则：每轮文件改动后，交付说明必须包含未提交改动的分组摘要与可直接执行的 `git add` / `git commit` 命令；AI 不执行提交。 |

| 证据ID | 内容 | 结果 |
|---|---|---|
| E-W12a | `windows-smoke.ps1` 新增 N20/N21 | **21/21 通过**；N20 `distinctFrames=14`，N21 `right=True left=True` |
| E-W12b | `verify-windows.ps1 -Full` | exit 0；cargo fmt/clippy/test/release、dotnet **30/30**、native smoke **21/21**（源码构建与解压包各一轮）、打包结构检查全绿 |
| E-W12c | 隔离诊断 `.scratch/diag18.ps1` | 右侧→`look-row-9`、左侧→`look-row-10`、移开→`idle` |
| E-W12d | smoke 前台锁判定 | N18/N19 本轮自动化均 PASS；若前台属于其它进程则 SKIP 并指向人工 W13 |

### B5 人工验收第七轮反馈与修复（2026-09-20）

| 反馈 | 根因 | 修复 |
|---|---|---|
| 1 拖动仍固定第一帧 | **app 实际加载的 `petsona_ffi.dll` 是旧 GNU 构建**：门禁检测到 VS 后构建的是 MSVC DLL，但 `verify-windows.ps1` 仍按“GNU 优先”挑候选，`Petsona.csproj` 也 GNU 优先复制；GNU DLL 停在 13:40，而 runtime 修复在 14:42–14:45。于是第六轮的 runtime 修复根本没有进入用户运行的进程，拖动时钟仍被每 80 ms 的 `SetState` 重置。 | 统一选源：`verify-windows.ps1` 选择本次实际构建的 DLL；`Petsona.csproj` 优先使用 `PETSONA_FFI_DLL`；`package-windows.ps1` 构建前选最新 DLL 并导出 `PETSONA_FFI_DLL`。Release 构建后新增 SHA256 哈希守卫，app 输出与本次构建 DLL 不一致时 gate 直接失败。 |
| 2 注视方向仍不完整，尤其鼠标在宠物上方不触发 | ① 与反馈 1 同一个旧 DLL（runtime 修复未生效）；② 上方不触发是注视椭圆余量问题：旧范围为短边 40% 进入 / 50% 释放，boba 192×208 时上缘外仅 76.8 px，光标在宠物上方 100 px 已超出触发区。 | ① 随 DLL 选源修复生效；② `GazeFilter` 触发余量翻倍：进入 80%、释放 100%（boba 192×208 时上/下/左/右额外 153.6 px，死区仍 22%）。 |
| 3 提交总结 | — | 保持 AGENTS.md 规则：每轮改动给出未提交摘要与 git 命令。 |

| 证据ID | 内容 | 结果 |
|---|---|---|
| E-W13a | DLL 时间戳对照 | GNU `target/x86_64-pc-windows-gnu/release/petsona_ffi.dll` = 13:40；MSVC `target/release/petsona_ffi.dll` = 17:15；runtime 源码 = 14:42–14:45；app / dist / 桌面包都含 13:40 旧 GNU DLL（9583104 B） |
| E-W13b | boba 隔离拖动诊断（新 DLL） | `windows-native-drag.log` 连续 `sprite=16→17→18→19→20→21→22→23→16…`，running-left 全 8 帧推进；旧 DLL 时停在 16 |
| E-W13c | boba 上方注视 | 翻倍前 `top-100` = idle；翻倍后 `top-100` = `look-row-9`、`top-60` = `look-row-9`；右 / 左仍分别 `look-row-9` / `look-row-10` |
| E-W13d | FFI 哈希守卫 + 最终门禁 | `verify-windows.ps1` Release 构建后断言 app 输出 DLL 的 SHA256 == 本次构建 DLL；最终 `-Full` 通过：cargo workspace、dotnet **31/31**、native smoke **21/21**（源码构建与解压包各一轮）、package smoke 21/21；E-W12b 当时的门禁没有覆盖这个选源错误 |
| E-W13e | smoke 判据修正 | N20 改为读取引擎发布的 sprite index（屏幕哈希会被窗口位移干扰，E-W12a 的 `distinctFrames=14` 不能作为拖动动画证据）；N9/N15 探测前重读窗口矩形；21/21 通过 |

### B5 人工验收第八轮反馈与修复（2026-09-20）

| 反馈 | 根因 | 修复 |
|---|---|---|
| 1（REQ-W04）拖动中方向变化响应不及时 | `PetWindow.OnMouseMove` 用 `cursor - dragStart` 的**总位移**判断方向：原地反向拖动时，只要光标还没越过最初按下点，状态一直是旧方向。 | 方向改用每帧 `cursor - lastMoveCursor` 的最近位移；反向累计 3 px 即立即发送新方向（绕过 80 ms TTL 刷新节流）。同时拖动期间暂停注视（`DragStarted` / `DragEnded` → `_draggingPet`），避免 gaze 优先级 20 覆盖 running 优先级 10。新增 N22：左拖 → 原地右拖，拖动日志同时出现 running-left（16..23）与 running-right（8..15）帧。 |
| 2（REQ-W09）注视方向抖动 / 监测范围体验 | ① 官方 Codex 实现（`app.asar` → `app-initial-*.js` `function oZc`）：**无最大半径**、离中心 1 px 死区、`atan2(dx,-dy)` 按 22.5° 量化为 16 方向；官方函数本身没有迟滞。② 我们旧映射用 `dy/|dx|` 截断，无法区分陡角的 22.5° 档位；旧死区只有短边 22%，光标靠近宠物时角度变化快，逐 poll 重发导致频繁翻帧。 | ① 共享引擎 `gaze_target` 改为官方 22.5° 映射（0°=上、顺时针、row9/row10、column=direction%8）；② Windows 前端新增 `GazeStabilizer`：官方量化 + 7° 角度迟滞 + 2 px 最小移动；光标在触发区内时每 poll 重发“保持方向”的单位向量，跨行多步过渡仍能完成；③ 内死区从 22% 扩大到 35%（boba 192×208 时约 67 px），外层保留翻倍后的触发椭圆（进入 80% / 释放 100% 余量）。官方范围结论：**无最大半径**，官方只有 1 px 死区；我们保留有界触发是为了避免全屏一直盯着光标。 |

| 证据ID | 内容 | 结果 |
|---|---|---|
| E-W14a | 官方实现溯源 | `app.asar` → `webview/assets/app-initial-6c4523b43a11.js` `function oZc`；常量 `sZc=22.5, cZc=16, lZc=9, uZc=8, dZc=1`；`Math.atan2(r,-i)`；无半径上限 |
| E-W14b | Rust 方向映射 | 新增 `gaze_target_matches_the_official_16_direction_table`（上 / 45° / 右 / 下 / 左 / 左上）；cargo core **57/57** |
| E-W14c | C# 稳定器 | 新增 GazeStabilizerTests（边界迟滞、2 px 下限、重置、单位向量回环）；dotnet **36/36**；deadzone 测试更新为 35% |
| E-W14d | smoke | N22 拖动反向通过（running-left/right 帧都出现）；**22/22 通过** |
| E-W14e | full gate | `verify-windows.ps1 -Full` exit 0：cargo workspace（core **57/57**、runtime 5/5、app 25/25）、dotnet **36/36**、native smoke **22/22**（源码构建与解压包各一轮）、FFI SHA256 守卫、打包结构全绿 |

### B5 人工验收第九轮反馈与修复（2026-09-21，启动慢 + 光标忙碌圈）

| 反馈 | 根因 | 修复 |
|---|---|---|
| 1 启动速度有一些慢 | 冷启动要把 WinUI 3 unpackaged 宿主（WindowsAppSDK 2.5 bootstrap）与 46 MB 托管依赖读进进程再 JIT；本地磁盘实测首次 3.2 s、之后热启动 0.55 s，从 WSL UNC 路径启动还要多约 1 s（每个 DLL 都走 9P 往返）。本轮只量化不改架构，优化选项见 CR-W1。 | 未改（等用户决定，见 CR-W1） |
| 2 启动后一段时间鼠标移到宠物上显示"加载中" | 宠物窗 / overlay 窗口类注册时 `WNDCLASSEXW.HCursor = NULL` 且窗口过程不处理 `WM_SETCURSOR`。Windows 对无类光标的窗口**不安装光标**，窗口上会一直保留进入前的形状：shell 启动程序期间设置的 `IDC_APPSTARTING`（忙碌圈）因此冻结在宠物上，直到鼠标移进另一个有光标的窗口。 | `PetWindow` / `OverlayWindow` 注册窗口类时设 `HCursor = IDC_ARROW`，并在 `WM_SETCURSOR` + `HTCLIENT` 时显式 `SetCursor(IDC_ARROW)` 后返回 1；托盘 owner 隐藏窗口一并补上类光标（MSDN 建议）。窗口类在**进程内**注册，所以这条只能由返回值/行为验证。 |
| 第九轮复测（2026-09-21，用户实机） | —— | 1 光标：**通过**（冷启动 / 热启动后立即把鼠标移到宠物与编辑按钮上，均为普通箭头，无忙碌圈残留）；2 启动速度：用户确认**可接受**，**CR-W1 选择选项 C**（不改代码，E-W15d 作为性能基线）。 |

| 证据ID | 内容 | 结果 |
|---|---|---|
| E-W15a | smoke N23/N24 判据改写 | 原判据用 `GetClassInfoExW` 读 app 的窗口类：窗口类是进程局部的，跨进程必然读不到（实测 `petClass=0`），所以第一版 N23 是假 FAIL / 修复后改判据又变成恒真假 PASS。改为对窗口发 `WM_SETCURSOR`（`HTCLIENT`）并断言窗口过程返回 1（自己接管光标）。**源码构建与解压包两轮各 24/24 通过**（N23 `result=1 control=0`、N24 `count=1 results=1 control=0`）。 |
| E-W15b | 判据自校准（负向对照） | 同一轮 smoke 现场注册一个"修复前形态"的隐藏窗口（类光标 NULL + `DefWindowProc`），它必须返回 0。实测 `control=0`、宠物窗与 overlay `result=1`（`.scratch/cursor-return-probe.ps1`：control=0、pet=1、overlay=1），证明判据能区分"光标无主"和"窗口接管"。 |
| E-W15c | 探针侧无法伪造光标（用来解释为何不用形状比对） | 实测 `SetCursor(APPSTARTING/WAIT)` 由探针进程调用时对他人窗口无效（连续 4 次读取仍 `65539=IDC_ARROW`），这是 `SetCursor` 的线程/窗口归属限制；因此形状比对在跨进程 smoke 里没有判别力，必须用返回值 + 对照窗口。 |
| E-W15d | 启动耗时量化（Release，隔离 home，同一构建） | ① 外部观感：`Start-Process` → 宠物窗可见：UNC 冷启动 4.17 s（health 3.92 s）；本地冷启动 3.18 s；本地热启动 0.58 / 0.54 s（overlay 约 1.05 s）。② 应用侧打点（临时 `PETSONA_STARTUP_TRACE`，测完已回滚并与 HEAD 逐字节一致）：热启动 0 ms App 构造 → 70 ms OnLaunched → 130 ms anchor 窗口 → 190 ms AppController 构造完（此时 PetWindow + 2×Overlay + Tray + Scheduler 都已创建）→ 285 ms 首帧可见；首次运行同为 539 ms。**结论：应用自身代码约 0.3 s（首次 0.54 s），冷启动其余 3–5 s 全在 CLR / WinUI / WindowsAppSDK 宿主初始化（进程创建 → App 构造之前），不在宠物窗或控制器**。脚本：`.scratch/startup-timing.ps1`、`.scratch/startup-trace.ps1`、`.scratch/build-trace.ps1`。 |
| E-W15e | full gate | `scripts\verify-windows.ps1 -Full` exit 0；cargo fmt/clippy/test/release、dotnet **36/36**、native smoke **24/24**（源码构建与解压包各一轮，共 48 个 PASS）、FFI SHA256 守卫、打包结构检查全绿。日志 `/tmp/verify-win.log`。 |
| E-W15f | 第九轮人工复测（用户实机，桌面验收包，隔离 home `%TEMP%\petsona-acceptance2`，端口 17873） | W14 人工通过：冷启动 / 热启动后光标为普通箭头，无 `IDC_APPSTARTING` 残留；启动速度用户确认可接受，CR-W1 决定选项 C（无代码 / 计划变更，性能基线即 E-W15d）。 |

### B5 人工验收第十轮复测（2026-09-21，W 矩阵收口）

用户实机复测（桌面验收包，隔离 home `%TEMP%\petsona-acceptance2`，端口 17873）：

| 项 | 结果 | 说明 |
|---|---|---|
| W2 托盘菜单 | 通过 | 溢出面板与固定到任务栏两种状态都能右键弹出菜单 |
| W3 点击 / 双击 / 拖动 | 通过 | 挥手 + 气泡、跳跃、跟手拖动与松手保存位置均正常 |
| W5 注视 | 通过 | 全方向跟随、范围、静止保持与死区 / 迟滞观感确认 |
| W6 气泡 / Composer | 通过 | Enter 发送、Shift+Enter 换行、Esc 关草稿、中文 IME 均正常 |
| W13 面板聚焦 | 通过 | Composer 与设置窗口都取得前台焦点，可直接键盘输入 |
| W1 窗口视觉 | 通过 | 透明 / 无边框 / 置顶，任务栏无多余窗口 |
| W4 穿透落点 | 通过 | 透明处落到桌面、不透明处仍可点击 |
| W7 设置页全分区 | 通过 | 导入导出删除、缩放、穿透、DeepSeek、人格、记忆、HKCU 自启开关 |
| W10 托盘退出 | 通过 | 进程结束、端口释放、无残留窗口 / 托盘 |
| W11 状态协议 | 通过（第二次复测） | 5 条步骤全部符合预期：`/health`、`/pets`、`waiting` 10 s 回退、`ttlMs:0` 粘滞、非法状态 400。复测中用户提出「第 4 条之后一直 running，点宠物 / 发聊天都不解除」——已确认是 `source` 生命周期 + 优先级仲裁的既定行为（见 CR-W2），同 source 再发一条即可解除 |

| 证据ID | 内容 | 结果 |
|---|---|---|
| E-W16a | 第十轮人工复测记录（用户实机描述，2026-09-21） | W1～W10、W13、W14 人工通过；W11 待用户按新落盘的「W11 手工步骤」确认；W12 按计划 v1.1 §3.1 暂缓。自动门禁未重跑（本轮只改文档，代码与 E-W15e 基线一致）。 |
| E-W16c | W11 第二次复测（用户实机，2026-09-21） | 5 条步骤全部符合预期，W11 人工通过；同时暴露 `ttlMs:0` 粘滞状态无法被点击 / 拖动 / 聊天解除的可用性缺口 → CR-W2（诊断依据：`state.rs::raise` 的 source/优先级仲裁、`session.rs::poll_state_events` 的 `hook:<source>` 前缀、`state_server.rs` 未使用 `action` 字段、`AppController` 的 `native` 源点击 / 拖动与 Composer 不推状态）。 |
| E-W16b | W11 手工步骤落盘 | `docs/WINDOWS_VERIFICATION.md` 新增「W11 手工步骤」小节：`/health`、`/pets`、`POST /state`（TTL 10 s 回退）、`ttlMs:0` 不过期、非法状态名 400；命令针对隔离实例端口 17873，不与日常实例 17872 冲突。 |

## 冲突与变更请求
| CR-W2 | 第十轮复测（W11 第 4 条）暴露可用性缺口：`ttlMs:0` 的协议状态由 `hook:<source>` 独占，用户点击（`native` 源 `waving` 40）、拖动（10）与 Composer 聊天（不推状态）都无法顶掉 `running`（70）；`StateEvent.action` 字段在 wire format 里存在但**只解析不生效**（`crates/petsona-core/src/state_server.rs`），core 已有 `clear_source` / `clear_all` 但未接到 FFI / UI / 协议。用户按 W11 步骤复测时宠物卡在 running，只能靠同 source 再发一条或重启应用解除。 | REQ-W08（协议）、REQ-W04/W07（交互）；计划 v1.1 无「解除粘滞状态」条目 | 选项 A：把已存在的 `action` 字段接上（`{"source":"x","action":"clear"}` → `clear_source("hook:x")`），零新依赖、兼容 UniPet 协议注释，需补 Rust + smoke 测试与文档；选项 B：托盘菜单加「恢复待机」（与「立即活动」对称，按 `clear_all` 实现）；选项 C：都不做，只在文档写明"同 source 再发一条 / 重启"的解法（现状即文档 W11 小节）。**建议 A（协议侧自洽）+ B 若希望纯 GUI 也能解除。** | 待用户决定（本轮未改代码） |



| CR | 文件事实 | 影响REQ/范围 | 建议 | 用户决定/计划版本 |
|---|---|---|---|---|
| CR-W1 | 第九轮用户反馈"启动速度有一些慢"。E-W15d 打点显示应用自身启动只占约 0.3 s（首帧 285 ms 热 / 539 ms 首次），冷启动其余 3–5 s 在 CLR + WinUI + WindowsAppSDK 宿主初始化；`AppController` 构造（PetWindow + 2×Overlay + Tray + Scheduler）约 60 ms；`Petsona.csproj` 未启用 `PublishReadyToRun`；SettingsWindow 已是按需创建。 | REQ-W01（构建/发布）、REQ-W13（打包脚本）；**计划 v1.1 无启动性能条目**，属新增范围 | 选项 A：`package-windows.ps1` publish 加 `-p:PublishReadyToRun=true`（只动发布包，减少 JIT；对数秒级宿主初始化估计帮助有限，须实测）；选项 B：延迟创建 overlay / tray —— 上限约 60 ms，收益太小，不建议；选项 C：不改代码，把 E-W15d 作为性能基线（热启动 0.5 s 已可接受）。**建议 C，若要压冷启动再单独试 A 并量化**。 | **用户 2026-09-21 决定：选项 C** —— 不改代码、不修订计划，E-W15d 作为性能基线；若日后仍要压冷启动，另立任务试选项 A 并量化。 |

## 审查及关闭

| REV/优先级 | REQ | 位置/触发/影响 | 必需修复与测试 | 关闭证据/状态 |
|---|---|---|---|---|
| — | 暂无（审查对话尚未开始） | — | — | — |

## 交付与接续

- 实际完成范围：计划 v1.1 落盘（含暂缓对齐）；批次 0 预研；**B0～B5 全部落地**（骨架/ABI、宠物窗与交互、设置页全量 + 自启、气泡/Composer/注视、故障/退出/位置持久化、脚本/打包/验收文档）；证据 E-W1～E-W6，第五轮交互修复见 E-W11a～e，第六轮交互修复见 E-W12a～d，第七轮 DLL 选源 / 注视范围修复见 E-W13a～e，第八轮拖动反向 / 官方注视映射见 E-W14a～e。
- 未完成/失败/SKIP/人工待验：W 矩阵人工项已闭合（W11 于第十轮第二次复测通过）；**CR-W2（协议粘滞状态的解除方式）待用户决定**；W12（多屏 / 重力 / 活动提醒 / 透明度 / 协议设置界面 / 托盘化 / 影子动画）按计划 v1.1 §3.1 暂缓；登录后实际自启与 D6 干净机器；CI 首次运行（待推送）；FFI panic 注入（无公开接口，两端共有）；共享 runtime 的重复 `SetState` 语义变更需 macOS 回归。
- 对用户现有数据/行为的影响：数据格式与协议未变，全部验证使用隔离 PETSONA_HOME；共享 runtime 行为有变更（重复同状态命令不再重置动画时钟、只刷新 TTL），macOS 需回归。
- 下一个执行者的安全接续点：**B6（清理）依赖 macOS 线完成（计划 REQ-W15 两端联动），当前不具备条件**；可先做 B6 的准备性审计（依赖清单），删除动作需两端验收闭环后再执行。
- 2026-09-21 人工验收进行中：首轮～第九轮反馈均已诊断修复（见「B5 验收反馈诊断与修复」「B5 人工验收第五轮反馈与修复」「B5 人工验收第六轮反馈与修复」「B5 人工验收第七轮反馈与修复」「B5 人工验收第八轮反馈与修复」）；桌面修复版验收包位于 `Desktop\Petsona-验收-修复版\`（隔离 home `%TEMP%\petsona-acceptance2`，端口 17873），第八轮后已刷新为 E-W14 构建。第九轮复测（2026-09-21）：W14 通过、启动速度用户确认可接受。
- Git操作是否发生（默认无）：无。
- 完成判定及对应证据：B5 端到端门禁与打包验证通过（E-W6a～d）；第五～第十轮修复与复测见 E-W11a～e、E-W12a～d、E-W13a～e、E-W14a～e、E-W15a～f、E-W16a～b；**W 矩阵除 W12（暂缓）外全部人工通过（W11 于第二次复测通过）**；启动性能按 CR-W1 选项 C 不改代码（E-W15d 为基线）。

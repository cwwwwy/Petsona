# windows-native-rewrite：执行契约

## 1. 身份与授权

- 任务 ID：`windows-native-rewrite`；版本：`1.1`；整理日期：2026-09-20（v1.1 对齐 macOS 暂缓清单，用户确认）。
- 本文展开 [native-ui-rewrite.md](native-ui-rewrite.md) v1.0 的 **REQ-16**（Windows 原生对等实现），不修改该计划已确认的 MAC/SHARED 需求；两条线并行执行。两者共同服务于最终架构：共享 Rust core/runtime/ffi + 各端原生前端。
- 用户确认决定（2026-09-20 对话）：
  1. 开始 Windows 线开发，与 macOS 线**并行推进**；macOS 线保持"代码完成、人工验收未完成"，不因开线改写状态。
  2. 旧 Windows 线（egui / `petsona-shell-windows`）的 W-* 实机复测**冻结**，观感/行为验收由新入口重新执行；旧问题不得因重写自动关闭。
  3. 授权按"批次 0 预研 → 落盘本计划"的路线执行；本文件即预研后的正式契约。
- 本次范围：`apps/windows` 原生前端（C# / WinUI 3 + Win32）、必要共享层/ABI 调整、Windows 验收脚本与发布门禁、验收文档。
- 非目标（本计划不授权）：Linux；macOS 产品代码改动（`apps/macos/**` 仅允许共享层接口同步适配）；提前删除 macOS 旧外壳；在 Windows 对等验收完成前删除旧 Windows 入口。
- 基线：`main` @ `c4fc413`，工作区干净（2026-09-20 核对）。
- 对应执行记录：[../execution/windows-native-rewrite.md](../execution/windows-native-rewrite.md)。

## 2. 批次 0 预研证据（2026-09-20，已完成）

环境事实（本机 Windows 11，WSL2 源码在 `/home/cwwwwy/Petsona`）：

| 项 | 事实 |
|---|---|
| OS | Windows 11 25H2，build 26200（满足 WinUI 3 要求） |
| .NET | SDK 10.0.401（`C:\Program Files\dotnet`）；WindowsDesktop runtime 10.0.12 |
| Windows SDK | 10.0.26100（含 MSIX/BuildTools） |
| Windows App Runtime | 已安装 1.7 / 1.8 / 2.4.0 / 2.5.1（unpackaged 运行依赖满足） |
| Visual Studio | **未安装**（无 VS 2022 / Build Tools）；预研证明不需要 |
| Windows App SDK | NuGet 稳定版 2.5.1（探针锁定并构建通过） |
| Rust（Windows 侧） | rustup：`1.98.0-x86_64-pc-windows-msvc`（含 gnu std）、`stable-*-gnu`、`stable-*-msvc`；无 MSVC `link.exe`，沿用 GNU target + rust-lld 回退（见 `docs/WINDOWS_VERIFICATION.md`） |
| 源码访问 | Windows 侧经 UNC `\\wsl.localhost\archlinux\home\cwwwwy\Petsona` 直接构建通过；不需要 Windows 侧源码副本 |

预研证据（探针 `.scratch/winui-probe`，gitignored，不入库）：

| 证据 | 内容 | 结果 |
|---|---|---|
| E-W0a | `dotnet build` 最小 WinUI 3 unpackaged 工程（net10.0-windows10.0.26100.0 + WindowsAppSDK 2.5.1） | **Build succeeded，0 警告 0 错误**（首次 restore 32.9s） |
| E-W0b | 运行探针 exe 6 秒 | RUNNING → STOPPED，unpackaged 应用可启动 |
| E-W0c | 从 Windows 本地 cwd 构建 UNC 源码路径 | Build succeeded（12.6s 增量），直连 WSL 源码可行 |
| E-W0d | `dotnet restore --use-lock-file` + `--locked-mode` | 两次 exit 0，锁文件门禁可行 |
| E-W0e | 运行通道 | `cmd.exe` 调用 dotnet 正常；`cmd.exe` 不能以 UNC 为 cwd，需先 `cd /d` 到本地再用绝对路径；Codex pwsh 通道 native 输出异常属工具通道问题，真实终端行为由执行批次复核 |

预研结论（本计划的既定决策）：

1. **无 Visual Studio 可行**：`dotnet` CLI + Windows App SDK 2.5.1 NuGet + Windows SDK 26100 完成构建与运行；不安装 VS，除非执行中遇到硬阻塞（届时提 CR）。
2. **首发形态为 unpackaged**（`WindowsPackageType=None`）：与旧线 HKCU Run 自启契约和 zip 便携分发一致；本机 Windows App Runtime 已满足运行。
3. **framework-dependent 起步**：是否 self-contained 由 B5 干净环境（D6）验收驱动决定。
4. **FFI 产物为 `petsona_ffi.dll`**（cdylib，GNU target 构建）随包分发；与 macOS 静态 `.a` 共用同一 `contracts/petsona.h`，ABI 版本与布局测试双端一致。
5. **源码单一来源**：源码保留在 WSL 仓库，Windows 侧用 UNC/映射盘符构建；不维护 Windows 侧手工副本（脚本 UNC 适配见 REQ-W13）。

## 3. 目标与需求

所有 REQ-W01～W14 为本线必需；REQ-W15 在对等验收通过后执行。

| REQ | 可验证要求 | 范围 | 完成标准 |
|---|---|---|---|
| REQ-W01 | `apps/windows` 工程可重现构建：sln / csproj / `global.json` / `Directory.Build.props` / `packages.lock.json`；`dotnet restore --locked-mode`、`dotnet build -c Release -p:Platform=x64`、`dotnet format --verify-no-changes --no-restore` 退出 0 | 本次 | 本地与 CI（windows-latest）一致；输入版本固定 |
| REQ-W02 | C# 用 `LibraryImport` 消费 `contracts/petsona.h` ABI 3；生命周期/所有权/故障处理与 macOS 相同；DLL 随包；不复制共享状态机 | 本次 | AbiTests（布局/版本）+ EngineClientTests（命令/销毁/故障注入）通过 |
| REQ-W03 | 宠物窗：Win32 HWND，透明/无边框/置顶/不激活/像素级穿透（当前帧 alpha + idle 并集回退）；精灵渲染（DirectComposition/D2D 或等价，执行者选型需测试证明）；无白线/闪框 | 本次 | 窗口属性探针 + smoke + M-W 人工 |
| REQ-W04 | 交互：单击/双击（320ms 窗口）/拖动区分；位置物理像素持久化；托盘图标（静态应用图标）+ 原生菜单（设置/换宠/显示隐藏/立即活动/退出）；不抢前台 | 本次 | smoke + M-W 人工 |
| REQ-W05 | 空库首启：无内置宠物；空库自动打开设置；文件夹/zip/Codex 显式导入、导出、删除、重复 ID 覆盖确认、切换 | 本次 | 导入/冲突/删除自动测试 + M-W |
| REQ-W06 | WinUI 3 设置页：缩放档位 0.5–2.0、穿透、DeepSeek 全配置 + Windows 凭据管理器、人格 CRUD/模板/导入导出、记忆管理、HKCU 自启、宠物库管理（协议设置界面与透明度设置按 §3.1 暂缓） | 本次 | 设置命令往返测试 + M-W |
| REQ-W07 | 气泡 / 编辑按钮 / Composer：IME、Enter 发送、Shift+Enter 换行、Esc 关闭、关闭保留草稿、caret 注视（影子→编辑按钮过渡动画按 §3.1 暂缓，按钮入口保留） | 本次 | 自动测试 + M-W（含中文 IME 人工） |
| REQ-W08 | 协议/单实例/日志/自启：`/state` `/health` `/pets` 兼容、TTL 经新入口、实例锁、日志文件、HKCU Run 写删 | 本次 | 新入口 smoke（含 TTL）+ 注册表测试 |
| REQ-W09 | 注视与调度：全局光标采样、椭圆触发/死区/迟滞/跨行、caret 目标、16ms 注视步进、按 deadline 调度、空闲 CPU 约 0–1% | 本次 | Rust 状态测试 + smoke + M-W 长测 |
| REQ-W10 | 物理像素位置持久化（单屏基础路径；重力、自动活动提醒、多屏/混合 DPI/工作区适配按 §3.1 暂缓） | 本次 | 位置保存/还原 + M-W |
| REQ-W11 | 故障与退出：panic 不跨 ABI、fault 后拒绝命令、stop 后无晚到回调、退出释放端口/锁、无残留窗口/托盘 | 本次 | 故障注入测试 + smoke |
| REQ-W12 | 测试隔离：测试宿主在初始化前隔离目录/端口/凭据/自启；生产包不可启用 test-hooks | 本次 | 隔离测试 + 发布包检查 |
| REQ-W13 | 脚本与打包：`verify-windows.ps1` / `windows-smoke.ps1` / `package-windows.ps1` 实际验证新入口；zip 分发；release workflow 更新；UNC 工作目录适配 | 本次 | 脚本 exit 0 + 包结构检查 + 干净目录启动 |
| REQ-W14 | 验收文档：`docs/WINDOWS_VERIFICATION.md` 新增新入口章节；W-* 冻结项映射到新线验收；人工矩阵执行并记录 | 本次 | 文档审查 + M-W 记录 |
| REQ-W15 | 对等验收通过后清理：删除 `petsona-shell-windows` 及旧 UI 依赖（`petsona-app` 与 macOS 线完成度联动） | 收尾 | 依赖检查无 egui/eframe/winit；两端发布包不含旧 UI |

### 3.1 暂缓范围（v1.1，对齐 macOS 线，用户 2026-09-20 确认）

用户要求 Windows 线对齐 macOS 开发中明确暂缓的功能清单（依据：`docs/execution/native-ui-rewrite.md` 8.9 节）。
以下项目**不在 B1–B6 的实现与验收范围**；解冻需用户明确决定并再次修订本计划：

| macOS 暂缓项（8.9） | Windows 对应 | 本线处理 |
|---|---|---|
| 1 自动活动提醒 | REQ-W10、REQ-W06 行为设置 | 暂缓（菜单「立即活动」保留基础接线，与 macOS 线当前行为一致） |
| 2 重力 | REQ-W10、REQ-W06 | 暂缓 |
| 3 native 状态协议设置界面 | REQ-W06 | 暂缓（runtime 协议服务本身保留，不受设置 UI 影响） |
| 11 宠物图标托盘化 | REQ-W04 | 暂缓（托盘使用静态应用图标） |
| 12 影子到编辑按钮动画 | REQ-W07 | 暂缓（编辑按钮入口本身保留） |
| 13 透明度设置 | REQ-W06 | 暂缓 |
| 14 多屏/Retina/Spaces/工作区适配 | REQ-W10（几何）、REQ-W03/W09 的跨屏部分 | 暂缓（单屏位置持久化与基础窗口行为保留） |

保留说明：位置记忆仍用物理像素；像素穿透、注视与 caret、缩放档位、气泡/Composer、宠物库、人格/记忆/DeepSeek 不受影响。

## 4. 逐文件变更

### 共享层与契约（仅必要调整）

| 路径 | 动作 | 具体改法 | REQ | 前置条件 |
|---|---|---|---|---|
| `contracts/petsona.h`、`contracts/ABI.md` | 修改 | 仅当 Windows 需要新命令/字段：只追加、ABI 版本递增、头/实现/两端同步 | W02 | 双端回归（见第 5 节） |
| `crates/petsona-ffi/**` | 修改 | 同上；保持薄转换层边界 | W02 | 同上 |
| `crates/petsona-runtime/**` | 修改 | 仅当 Windows 平台服务（凭据/HKCU 等）需要 runtime 调整 | W06/W08 | 同上 |
| `.gitignore` | 修改 | 增加 .NET 构建产物忽略（`bin/`、`obj/`、`*.user` 等，不忽略工程与锁文件） | W01 | 无 |
| `.github/workflows/ci.yml` | 修改 | 增加 Windows 原生 build/test job（Rust windows target + dotnet） | W01/W13 | 工程骨架就位 |

### Windows 原生前端（新增，`apps/windows/`）

| 路径 | 职责 | REQ |
|---|---|---|
| `Petsona.sln`、`global.json`、`Directory.Build.props`、`.editorconfig` | 解决方案、SDK 固定、分析器与格式 | W01 |
| `Petsona/Petsona.csproj`、`packages.lock.json`、`app.manifest`、`App.xaml(.cs)` | WinUI 入口、DPI 感知、生命周期 | W01/W06 |
| `Petsona.Core/Petsona.Core.csproj`、`Interop/NativeMethods.cs`、`Interop/EngineHandle.cs`、`EngineClient.cs`、`AppState.cs` | ABI 绑定、命令/快照/事件、状态投影（拒绝旧修订） | W02 |
| `Petsona/Native/WindowCoordinator.cs`、`PetWindow.cs`、`OverlayWindow.cs`、`ComposerWindow.cs` | 窗口/浮层/焦点/输入与 IME | W03/W04/W07 |
| `Petsona/Native/PointerService.cs`、`MonitorService.cs`、`TrayService.cs`、`MenuService.cs`、`SystemServices.cs` | 全局光标、DPI/工作区、托盘、菜单、文件/凭据/HKCU | W04/W08/W09/W10 |
| `Petsona/Rendering/SpriteRenderer.cs`、`FrameScheduler.cs` | 精灵渲染、资源缓存与代次、deadline 调度 | W03/W09 |
| `Petsona/Views/SettingsWindow.xaml(.cs)`、`PetLibraryPage.xaml(.cs)`、`ViewModels/*` | 设置/宠物库/人格/记忆/DeepSeek 页面 | W05/W06 |
| `Petsona/Diagnostics/TestProbe.cs` | 测试探针（仅测试构建可用） | W12 |
| `Petsona.Tests/Petsona.Tests.csproj`、`AbiTests.cs`、`EngineClientTests.cs`、`CoordinateTests.cs`、`StateProjectionTests.cs` | 布局/生命周期/几何/状态投影测试 | W02/W10/W12 |

### 脚本、CI 与文档

| 路径 | 动作 | 具体改法 | REQ |
|---|---|---|---|
| `scripts/verify-windows.ps1` | 修改 | 切到新入口门禁：Rust workspace + `petsona_ffi.dll` + dotnet build/test + smoke + 包检查；兼容 UNC/网络路径（映射盘符或本地 cwd + 绝对路径） | W13 |
| `scripts/windows-smoke.ps1` | 修改 | smoke 针对新入口重写并扩充（协议/TTL、窗口属性、托盘、穿透、单实例、退出） | W13 |
| `scripts/package-windows.ps1` | 修改 | 打包新入口（exe/dll/资源/VERSION/README），结构校验 | W13 |
| `.github/workflows/release-windows.yml` | 修改 | 新入口构建与产物 | W13 |
| `docs/WINDOWS_VERIFICATION.md` | 修改 | 新增新入口验收章节与人工矩阵；旧章节标历史/冻结 | W14 |
| `docs/WINDOWS_ISSUES.md`（2026-09-21 归档到 `docs/archive/`） | 修改 | W-* 标注冻结，映射到新线验收项 | W14 |
| `AGENTS.md`、`README.md`、`docs/PLATFORM_ARCHITECTURE.md` | 修改 | 状态与任务入口同步 | W14/W15 |
| `crates/petsona-shell-windows/**`、旧共享 UI | 删除（W15 阶段） | 对等验收通过后与 macOS 线联动清理 | W15 |

## 5. 必须保持的约束

1. **并行线规则（新）**：macOS 线保持"代码完成、人工验收未完成"口径；Windows 线进展不改写 macOS 验收状态。共享层/ABI/契约任何变更：布局或所有权变化必须递增 ABI 版本，并完成两端验证——Rust 门禁 + macOS 原生门禁（Mac 上 `verify-macos-all.sh`，无法立即执行时标记"待回归"并禁止任一端的发布）+ Windows 构建/测试。
2. **旧线冻结语义（新）**：`petsona-shell-windows` 停止功能开发与人工复测，保持"可构建 + 现有自动门禁通过"直到新线接入脚本；`docs/archive/WINDOWS_ISSUES.md` 的 W-* 不再变更状态；观感/行为类关注点（注视流畅、气泡/输入框/影子、重力、活动、缩放）由新线 M-W 人工项重新验收，旧问题不得因重写自动关闭。
3. **数据与兼容（沿袭）**：数据目录 `%APPDATA%\Petsona` 与 `PETSONA_HOME` 不变；无内置宠物；旧 `config.json` schema 兼容；协议仅 loopback、默认 17872、`ttlMs: 0` 不过期；凭据账户 `com.petsona.desktop` / `deepseek`（Windows 走凭据管理器）；密钥不进日志/快照。
4. **窗口与焦点**：宠物/气泡/托盘不抢前台（`WS_EX_NOACTIVATE`）；设置/Composer 可激活；点菜单外不引发额外挥手；菜单 Esc/点外可关闭。
5. **几何**：位置记忆用物理像素；工作区夹取（`MonitorFromPoint` + `GetMonitorInfoW(rcWork)`）；混合 DPI 与多显示器不漂移。
6. **输入**：Enter 发送、Shift+Enter 换行、Esc 关闭；IME 组合期间 Enter 只提交候选；关闭保留草稿。
7. **资源与线程**：网络/磁盘不阻塞 UI 线程；图集按代次传递不逐帧复制；panic 不跨 ABI；stop 后无晚到回调；UI 仅主线程使用句柄。
8. **测试隔离**：测试宿主在入口初始化前隔离目录/端口/凭据/自启；生产包不能启用 test-hooks；不得删除失败测试或用 SKIP 充数。
9. **Git 规则**：只读查询；add/commit/push/tag 由用户执行（沿袭 AGENTS.md）。

## 6. 验收与命令

区分「现有命令」（本计划落地后可立即跑）与「实施后命令」（对应 REQ 完成后才存在）。

| ID | REQ | 目标/环境 | 命令 | 预期/证据 |
|---|---|---|---|---|
| T-WR0 | 全 | WSL 或 Windows 上的仓库 | `cargo fmt --all -- --check`；`cargo clippy --workspace --all-targets --locked -- -D warnings`；`cargo test --workspace --locked` | 退出 0；记录测试数 |
| T-WR1 | 并行规则 | Mac（共享层变更后） | `bash scripts/verify-macos-all.sh --gates-only`（必要时全量） | 退出 0，或明确"待回归"并禁止发布 |
| T-WR2 | W02 | Windows | `cargo build -p petsona-ffi --release --target x86_64-pc-windows-gnu`（GNU 回退环境变量见 `docs/WINDOWS_VERIFICATION.md`） | 产出 `petsona_ffi.dll` |
| T-WB | W01 | Windows（本地 cwd + UNC/盘符源码路径） | `dotnet restore --locked-mode`；`dotnet build -c Release -p:Platform=x64`；`dotnet format --verify-no-changes --no-restore` | 退出 0；0 警告目标 |
| T-WT | W02–W12 | Windows | `dotnet test -c Release -p:Platform=x64` | 测试数与退出 0 |
| T-WS | W03–W13 | Windows 桌面会话 | `powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1 -Full`（改造后） | smoke 全绿；SKIP 写明原因 |
| T-WP | W13 | Windows | `powershell -ExecutionPolicy Bypass -File scripts\package-windows.ps1`；解压到干净目录启动 | 包结构 + 隔离运行通过 |
| M-W | W03–W14 | 真实桌面（人工） | 按 `docs/WINDOWS_VERIFICATION.md` 新章节执行 | 逐项记录机器/系统/架构 |

## 7. 批次顺序

| 批次 | 内容 | REQ | 退出条件 |
|---|---|---|---|
| B0 | 工程骨架、锁文件、CI、ABI 绑定与测试基线 | W01/W02/W12 | T-WB/T-WT 绿；AbiTests 通过 |
| B1 | 最小可用：宠物窗/渲染/穿透、托盘菜单、空库首启、协议 | W03/W04/W05/W08 | 可导入并显示宠物；T-WS 基础项通过 |
| B2 | 设置与业务全量：DeepSeek/凭据、人格、记忆、宠物库管理 | W06 | 设置命令往返测试 + 导入导出删除链路 |
| B3 | 浮层与输入：气泡/编辑按钮/Composer/IME、注视与调度 | W07/W09 | 输入与注视测试 + M-W 草测 |
| B4 | 故障与退出、位置持久化基础（重力/自动活动/多屏按 §3.1 暂缓） | W10（收缩）/W11 | 位置保存 smoke + 故障测试 + M-W |
| B5 | 门禁与交付：脚本改造、打包、CI、验收文档与人工矩阵 | W13/W14 | T-WS/T-WP + M-W 完整记录 |
| B6 | 清理旧线（需两端条件满足后单独确认） | W15 | 依赖检查无旧 UI |

## 8. 暂停与完成

- **必须暂停并交回用户的冲突**：共享层/ABI 变更无法双端回归；数据格式/兼容变更；自启或安装契约变化（如改 MSIX）；需要引入 Visual Studio 等新工具链；Windows App SDK 版本策略影响已验收 macOS；发现计划外范围（Linux 等）。
- **执行者可自行判断**：Win32 互操作实现方式（手写 `LibraryImport` / CsWin32）；渲染实现细节（DirectComposition/D2D 组合，须以 REQ-W03 测试证明）；设置页布局与文案；测试组织方式。
- **本次完成条件**：REQ-W01～W14 每项（按 §3.1 暂缓范围收缩后）具备实现、自动验证与人工验收证据（人工可按环境分批，但必须闭合）；macOS 线未破坏且有回归记录；旧入口未提前删除。
- **全项目完成条件**：本计划全部完成 + macOS 线闭环 + REQ-W15 清理完成，两者共同关闭 native-ui-rewrite REQ-16。

## 9. 修订记录

| 版本 | 用户确认依据 | 改变 |
|---|---|---|
| 1.0 | 2026-09-20：用户确认并行推进、冻结旧线复测、授权"预研后落盘" | 初版：展开 REQ-16 为 REQ-W01–W15，引入并行线规则与旧线冻结语义 |
| 1.1 | 2026-09-20：用户要求对齐 macOS 暂缓清单后再执行 B1–B6 | 新增 §3.1：自动活动提醒、重力、协议设置界面、宠物图标托盘化、影子动画、透明度设置、多屏/工作区适配暂缓；收缩 REQ-W04/06/07/10 与 B4 范围 |

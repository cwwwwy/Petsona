# Petsona 原生 UI 重构：执行契约

## 1. 身份、授权与基线

- 任务 ID：`native-ui-rewrite`；计划版本：`1.0`；整理日期：2026-09-19。
- 本文整理先前对话中的完整计划和用户确认，取代旧的概要计划文档。本文描述要求，不记录完成状态。
- 用户确认：采用共享 Rust 核心 + 平台原生 UI，不要求 Rust-only；直接面向最终架构，不发布中间混合产品。
- 用户随后授权：先把计划写成文档，先实施 macOS 环境下的开发。当前范围为 macOS 及必要共享层；不是仅交付骨架。Windows 后续，Linux 不在范围。
- 本轮用户授权仅整理协作文档；不能据此自动恢复产品代码实施。另一个执行对话引用本文并获得执行请求后按本文继续。
- 基线：`main`，HEAD `ad045bb`。工作区已有未提交实现，不是干净 checkout。tracked 改动包括 AGENTS、Cargo.toml/lock、README、runtime/logging.rs、架构/验收文档和 verify-macos-all.sh；未跟踪目录包括 `apps/macos`、`contracts`、`crates/petsona-ffi` 及新增文档。
- 接手时必须读取实际 diff 和未跟踪文件，记录 HEAD/工作区摘要；不得重建并覆盖上一轮改动。审查发现见执行记录 REV-01–REV-09。
- 需求语义来自用户对话；执行者不能用修改本文或执行记录来降低标准。重大修订记录用户决定、版本和受影响 REQ。

## 2. 交付目标与依赖

最终：Rust core/runtime/ffi → Windows C#/WinUI 3/Win32、macOS SwiftUI/AppKit。每端单进程；Rust 管业务，原生前端管 UI 主线程、窗口、控件和渲染。

当前 macOS 交付必须能完整使用既有功能，并完成原生自动化与实机验收。旧 egui 入口在迁移期间可保留作对照；macOS 新入口通过后替换 macOS 产品启动/打包。全仓删除 `petsona-app` 和两个旧 shell 要等 Windows 也完成，不得为 macOS 迁移破坏仍在使用的 Windows 入口。

依赖顺序：基线和功能映射 → 共享业务边界 → FFI/静态链接 → macOS 原生窗口与功能 → 原生测试/打包 → macOS 切换。Windows 和全仓清理是后续工作，不以它们未完成阻止当前 macOS 工作，但不能宣称整个重构完成。

## 3. 要求与验收矩阵

所有 MAC/SHARED 项为当前交付必需；GLOBAL 项属于后续范围。测试编号对应第 7 节。

| ID / 范围 | 必须交付 | 验收证据 |
|---|---|---|
| REQ-01 SHARED | core 无 UI；runtime 封装应用状态/命令/任务；FFI 不再直接实现业务或在 UI 线程读写磁盘 | T-R、T-F；依赖检查和慢 IO 响应测试 |
| REQ-02 SHARED | 有版本/大小契约的 C ABI、所有权、线程规则、事件唤醒、取消/停止/销毁、故障后拒绝调用 | T-F、T-S；跨语言布局、缓冲区、故障与生命周期测试 |
| REQ-03 MAC | Rust 静态库按实际构建架构链接；无仓库路径动态依赖；可在干净目录启动 | T-B、T-P；otool、架构检查与移动包启动 |
| REQ-04 SHARED | 旧配置/人格/记忆、数据路径、凭据标识兼容；存储失败不损坏原文件 | T-R、T-S；旧数据夹具、失败写入、凭据读取 |
| REQ-05 MAC | 本地宠物库导入/导出/覆盖/删除/切换，Codex 显式导入；V1/V2 校验；无内置宠物 | T-R、T-S、T-N、M-01 |
| REQ-06 MAC | firstRun/空库自动开设置；无宠物/影子/问候/气泡；可从原生界面完成首次导入 | T-S、T-N、M-01 |
| REQ-07 SHARED/MAC | 原动画、16ms 注视步进、死区/迟滞/跨行、caret 目标；协议 TTL 到期回退 | T-R、T-F、T-N、M-02；必须经新入口测试 TTL |
| REQ-08 MAC | 单击/双击/拖动区分；像素穿透含 idle 并集回退；设置/菜单/宠物焦点规则 | T-R、T-S、T-N、M-02 |
| REQ-09 MAC | 物理像素位置记忆、工作区、缩放锚点/过渡、重力落地、活动提醒及立即活动 | T-R、T-S、T-N、M-03 |
| REQ-10 MAC | 气泡/影子/编辑按钮/原生输入完整；IME、选区、撤销、快捷键、关闭保留草稿、caret 注视 | T-S、T-N、M-04 |
| REQ-11 MAC | 全部设置、人格和记忆管理、DeepSeek 问候/对话及本地回退，后台请求不阻塞 UI | T-R、T-S、T-N、M-01/M-04；网络使用 stub |
| REQ-12 SHARED/MAC | /state、/health、/pets 兼容；单实例、日志、托盘菜单、LaunchAgent、Keychain | T-R、T-F、T-N、M-05 |
| REQ-13 MAC | 静止/空库/失败/隐藏无无意义高频循环；输入/协议可及时唤醒；安全退出、资源回收 | T-F、T-S、T-N、M-06 |
| REQ-14 SHARED/MAC | 测试宿主初始化前隔离目录/端口/凭据/自启；生产构建不带测试控制通道 | T-F、T-S、T-N、T-P；不能只隔离测试方法内的实例 |
| REQ-15 MAC | 统一脚本实际测新入口；原生打包、版本/图标、依赖检查和发布工作流；签名/公证有真实结果 | T-G、T-P、M-05；凭据不足保持待验收 |
| REQ-16 GLOBAL | Windows 原生对等实现；两端通过后移除所有旧 UI/外壳与产品中的 egui/eframe/winit | 后续 Windows 原生测试/完整脚本、两端依赖检查 |
| REQ-17 SHARED | 功能表、旧测试迁移映射、执行记录真实；每项要求均有位置、结果和未完成项 | 文档链接/映射审查；无用旧测试冒充新验收 |

## 4. 逐文件修改：共享层

本节路径均相对于仓库根。现存雏形必须修正后接入最终边界；新模块名按本表组织。局部函数/私有类型可自行选择，变更架构或数据契约必须提出计划变更。

| 路径 | 动作与改法 | REQ |
|---|---|---|
| `Cargo.toml`、`Cargo.lock` | 增加 ffi，迁移依赖；FFI unwind + catch 边界一起验证；当前保留旧 Windows 需要的成员，最终按 REQ-16 清理 | 01/02/16 |
| `rust-toolchain.toml` | 精确锁定编译器并与 CI 一致，明确目标架构 | 03/15 |
| `.gitignore` | 忽略 .NET/Xcode 构建缓存和测试产物，不忽略工程、锁文件、公共头和共享 scheme | 15 |
| `crates/petsona-core/Cargo.toml`、`src/lib.rs` | 保留模型/解析依赖，文件网络服务迁出；调整模块导出 | 01 |
| `crates/petsona-core/src/config.rs` | 保留 DTO、默认值/schema/校验；路径和持久化移至 runtime；旧 JSON 不变 | 04 |
| `crates/petsona-core/src/error.rs` | 保留业务错误；平台/ABI 错误由外层转换 | 01/02 |
| `crates/petsona-core/src/persona.rs`、`src/memory.rs` | 保留模型、提示/记忆规则，提取文件 IO；不丢已有测试 | 04/11 |
| `crates/petsona-core/src/pet/state.rs` | 保留优先级、动画/注视规则；注入可控时间，确保新引擎执行 TTL tick | 07 |
| `crates/petsona-core/src/pet/atlas.rs`、`src/pet/manifest.rs`、`src/pet/mod.rs` | 保留解析/帧表/掩码，外部读取字节；暴露与 GPU 无关的数据，保持格式和安全校验 | 05/07/08 |
| `crates/petsona-core/src/pet/library.rs` | 文件库逻辑移至 runtime/library.rs，再删除原实现，更新所有调用方 | 05 |
| `crates/petsona-core/src/deepseek.rs` | 网络移至 runtime，纯提示/清理移至 prompt.rs，再删除旧实现 | 11 |
| `crates/petsona-core/src/state_server.rs` | HTTP 服务迁至 runtime；DTO/验证移至 protocol.rs，再删除旧实现 | 07/12 |
| `crates/petsona-core/src/secrets.rs` | 系统凭据由平台服务实现；脱敏移至 runtime；测试内存 store 可保留在测试支持中；更新调用后删除旧生产实现 | 04/11/14 |
| `crates/petsona-core/examples/pet_inspect.rs` | 工具自行读文件并调用纯解析，不反向依赖 runtime | 01/05 |
| 新增 `crates/petsona-core/src/geometry.rs` | 点/矩形、工作区夹取、缩放锚点 | 09 |
| 新增 `crates/petsona-core/src/interaction.rs` | 点击/拖动/菜单守卫、注视目标规则 | 07/08 |
| 新增 `crates/petsona-core/src/motion.rs`、`src/clock.rs` | 前者重力/行走/缩放曲线，后者单调时间抽象；不按绘制次数推进业务 | 07/09/13 |
| 新增 `crates/petsona-core/src/protocol.rs`、`src/prompt.rs` | 前者协议 DTO/TTL，后者纯提示与文本清理 | 07/11/12 |
| `crates/petsona-runtime/Cargo.toml`、`src/lib.rs` | 接收 IO/网络依赖，默认关闭 test-hooks；仅导出服务入口 | 01/14 |
| `crates/petsona-runtime/src/session.rs` | 收起公共可变字段；状态仅由引擎命令修改，确保对话/切宠/协议统一更新 | 01/07/11 |
| `crates/petsona-runtime/src/pet.rs` | 资源加载/缓存和代次；帧描述与下次 deadline；前端不重复解码业务资源 | 03/05/07/13 |
| `crates/petsona-runtime/src/greeting.rs` | 本地回退、超时和旧请求代次失效 | 11 |
| `crates/petsona-runtime/src/instance_lock.rs`、`src/logging.rs` | 前者覆盖完整引擎生命期；后者进程内幂等初始化、准确反馈、日志脱敏，不误报绑定了另一个目录 | 12/14 |
| 新增 `crates/petsona-runtime/src/engine.rs` | 串行状态执行器、启停/故障；后台结果提交回引擎 | 01/02/13 |
| 新增 `crates/petsona-runtime/src/commands.rs`、`src/events.rs`、`src/snapshot.rs` | 分别定义命令、带序号事件和不可变快照；密钥不出现在快照 | 01/02/11 |
| 新增 `crates/petsona-runtime/src/scheduler.rs`、`src/tasks.rs` | 前者汇总动画/TTL/活动 deadline 与输入唤醒；后者后台 IO、取消、超时、代次，停机后不访问前端 | 07/11/13 |
| 新增 `crates/petsona-runtime/src/paths.rs`、`src/persistence.rs` | 保持数据路径；原子保存配置/人格/记忆及迁移失败保护 | 04 |
| 新增 `crates/petsona-runtime/src/library.rs`、`src/deepseek.rs`、`src/state_server.rs` | 分别承接宠物库、网络客户端、HTTP 服务，统一进入引擎，不阻塞 UI | 05/11/12 |
| 新增 `crates/petsona-runtime/src/platform_requests.rs`、`src/test_hooks.rs` | 前者异步凭据等平台请求/结果；后者 feature-gated 测试服务和原生探针 | 02/12/14 |

共享模块迁移必须同步更新尚存旧入口的 import/调用以维持 Windows 可构建；这是兼容接线，不新增旧 UI 功能。

### FFI 与契约

| 路径 | 动作与改法 | REQ |
|---|---|---|
| `crates/petsona-ffi/Cargo.toml` | rlib/cdylib/staticlib，依赖 runtime；显式测试 feature，生产默认无探针 | 02/03/14 |
| `crates/petsona-ffi/src/lib.rs` | 拆开现有单文件；只导出 ABI 和薄转换，停止直接改 runtime 字段 | 01/02 |
| 新增 `src/types.rs`、`src/handles.rs`、`src/buffers.rs`（上述 crate 内） | 固定布局/版本/长度；句柄与故障状态；UTF-8/复制/释放规则及缓冲区边界 | 02 |
| 新增 `src/commands.rs`、`src/events.rs`、`src/render.rs`、`src/error.rs` | 命令校验；事件读取/唤醒；资源代次/帧/掩码；错误与 panic 隔离 | 02/07/08/13 |
| 新增 `build.rs`、`cbindgen.toml`（上述 crate 内） | 生成公共头并检测漂移，不以手写双份声明代替一致性验证 | 02 |
| `contracts/petsona.h`、`contracts/ABI.md` | 写实可验证的版本/结构大小、所有权、线程、取消/stop/销毁/故障契约；Swift 和未来 C# 共用 | 02 |
| 新增 `contracts/COORDINATES.md`、`contracts/BEHAVIOR.md` | 坐标与旧位置转换、点击/注视/重力/菜单/输入规则；不将实现特定标志当产品约束 | 07/08/09/10 |

## 5. 逐文件修改：macOS

现有 Swift 文件集中在 `apps/macos/Petsona/Sources`；保留此源根并按下列子目录整理，不复制一套同名入口。

| 路径（相对于 `apps/macos`） | 动作与改法 | REQ |
|---|---|---|
| `project.yml` | 源工程定义；静态库用明确 .a 路径，按 Xcode 实际架构构建 Rust；隔离测试宿主；依赖/输入输出和签名配置明确 | 03/14/15 |
| `Petsona.xcodeproj/project.pbxproj`、`project.xcworkspace/contents.xcworkspacedata`、`xcshareddata/xcschemes/Petsona.xcscheme` | 从 spec 生成并提交，测试 scheme 真正执行测试；Release 不混入测试 target；不能只改生成文件 | 03/14/15 |
| 新增 `Config/Base.xcconfig`、`Config/Release.xcconfig` | 分别管理架构/最低系统/警告/版本、发布与签名参数 | 03/15 |
| `Petsona/Info.plist` | 与 packaging 模板选定一个源，保留 bundle ID、LSUIElement/Retina、图标/版本；不维护冲突两份 | 15 |
| 新增 `Petsona/Petsona.entitlements` | 明确所需权限，未经决定不扩大权限、不开启不兼容 sandbox | 12/15 |
| `Petsona/Sources/PetsonaApp.swift`、`AppDelegate.swift` | 唯一入口/主循环；初始化前判断隔离测试环境，处理单实例失败、空库开设置、显式 stop；移出业务逻辑和轮询计时 | 06/12/13/14 |
| `Petsona/Sources/EngineClient.swift` → `Bridge/EngineClient.swift`；新增 `Bridge/module.modulemap` | 同一 C 头导入；命令/事件/资源/版本封装；显式主线程生命周期，不用 unsafe 注解代替约束 | 02 |
| 新增 `Petsona/Sources/Model/AppState.swift` | 主线程可观察快照/请求状态，拒绝旧修订；不复制 Rust 状态机 | 01/02 |
| `Petsona/Sources/PetWindowController.swift` → `Windows/WindowCoordinator.swift`、`Windows/PetPanel.swift` | 前者管理窗口锚点/可见性/实际几何回报，后者非激活宠物窗；删除整窗穿透误实现及 Swift 内点击业务 | 08/09/13 |
| 新增 `Petsona/Sources/Windows/OverlayPanel.swift`、`Windows/ComposerPanel.swift` | 气泡/影子非激活；输入框可激活，正确关闭保留草稿，避免抢焦点循环 | 10 |
| 新增 `Petsona/Sources/Views/ComposerTextView.swift` | NSTextView，marked text/选区/撤销/快捷键/caret 坐标；IME 提交不发送 | 10 |
| 新增 `Petsona/Sources/Rendering/SpriteView.swift`、`Rendering/FrameScheduler.swift` | 从旧 PetView 提取；缓存图集/裁剪/alpha/比例/缩放；按 deadline 刷新并响应事件，空库/失败停止高频循环 | 03/07/09/13 |
| 新增 `Petsona/Sources/Platform/PointerService.swift`、`Platform/MonitorService.swift` | 输入/掩码命中/事件监听释放；工作区/Retina/负坐标/逐屏换算 | 07/08/09 |
| 新增 `Petsona/Sources/Platform/TrayMenuService.swift`、`Platform/SystemServices.swift` | 托盘图标/菜单命令；文件面板/Keychain/LaunchAgent/打开目录，异步结果交回核心 | 05/12 |
| `Petsona/Sources/SettingsView.swift` → `Views/SettingsView.swift`；新增 `Views/PetLibraryView.swift` | 原生完整设置与宠物管理；不可用功能不伪装已可用；绑定共享服务 | 05/06/11 |
| 新增 `Petsona/Sources/Diagnostics/TestProbe.swift` | 测试构建专用：窗口/焦点/几何/重绘/输入；正式包不可开启 | 14 |
| `PetsonaTests/EngineClientTests.swift` | 扩展真实 Rust 调用、命令、旧数据、销毁重建；先隔离宿主再测试 | 02/04/14 |
| 新增 `PetsonaTests/AbiTests.swift`、`CoordinateTests.swift`、`StateProjectionTests.swift` | 分别验证 C/Rust/Swift 布局与错误、屏幕转换、事件顺序与旧结果丢弃 | 02/09/13 |
| 新增 `PetsonaUITests/SettingsTests.swift`、`ComposerTests.swift`、`WindowLifecycleTests.swift` | 原生首次导入/设置、输入框/焦点、反复开关/隐藏/切宠；接入 Xcode | 05/06/08/10/13 |
| `README.md` | 可复现构建/测试步骤、前置工具、产物路径、当前限制；不把骨架说成完整产品 | 17 |

## 6. 测试、脚本、旧入口及后续文件

| 文件 | 改法 | REQ |
|---|---|---|
| 新增 `crates/petsona-core/tests/interaction.rs`、`geometry.rs` | 迁移旧 UI 中纯规则回归，测试单双击/拖动/菜单、注视、掩码和几何 | 07/08/09 |
| 新增 `crates/petsona-runtime/tests/scenarios.rs`、`compatibility.rs`、`lifecycle.rs` | 业务链路；旧数据/失败保存；并发结果/取消/单实例/端口释放 | 04/05/07/11/12/13 |
| 新增 `crates/petsona-ffi/tests/abi.rs`、`lifecycle.rs` | 精确布局、UTF-8/缓冲区、错误版本/数值、panic 终止、stop/销毁、重建与晚到结果 | 02/14 |
| `scripts/verify-macos-all.sh` | 唯一入口：Rust gates + 新 Xcode tests/build + 原生 smoke + 新包依赖/结构检查；不同入口结果明确区分 | 03/14/15 |
| `scripts/macos-smoke.sh` | 启动新的 .app，临时 home/端口/token；协议、窗口、输入、调度、资源检查；不再拿旧 shell 代表新 UI | 07–15 |
| `scripts/package-macos.sh` | 构建同架构 Rust .a + Xcode app；使用最终版本/图标；复制可独立运行的新 app；检查所有 Mach-O 依赖 | 03/15 |
| `scripts/sign-macos.sh`、`notarize-macos.sh` | 适配最终 bundle、内到外签名、公证/staple；凭据缺失如实记录 | 15 |
| `scripts/install-macos-launch-agent.sh`、`packaging/macos/com.petsona.desktop.plist` | 保留 LaunchAgent 标识，更新最终可执行路径，不切换登录项机制 | 12 |
| `packaging/macos/Info.plist` | 已清理：native Xcode plist `apps/macos/Petsona/Info.plist` 是唯一 bundle 信息源 | 15 |
| `.github/workflows/ci.yml` | main push、Rust 两端检查和 macOS 原生 build/tests/ABI gates；Windows 旧入口暂保留 | 15/16 |
| `.github/workflows/release-macos.yml` | 新产物/门禁；手动产 artifact，正式 tag 发布；真实签名与未签名包区分 | 15 |
| `AGENTS.md`、`README.md`、`docs/PLATFORM_ARCHITECTURE.md`、`docs/MACOS_VERIFICATION.md` | 更新长期决策/入口/环境要求，目标与现状分开，引用执行记录 | 17 |
| `docs/FEATURE_PARITY.md`、`docs/execution/native-ui-rewrite.md` | 逐 REQ 更新真实状态/测试证据/审查关闭记录，不能篡改计划 | 17 |
| `docs/WINDOWS_VERIFICATION.md`、`docs/archive/WINDOWS_ISSUES.md`、`CHANGELOG.md` | 保留历史验收，记录共享层影响和新版本变化；旧问题不得因重写自动关闭 | 16/17 |

旧文件迁移归属：`crates/petsona-app/src/app.rs` 的生命周期至前端，调度至 runtime；`app/interaction.rs`、`app/geometry.rs` 的纯规则至 core；`app/pets.rs`、`app/conversation.rs` 的业务至 runtime；`app/settings.rs`、`bubble.rs`、`shadow.rs`、`menus.rs` 的 UI 至原生前端；`src/test_hooks.rs`、`app/test_hooks.rs` 至 runtime 测试服务与原生探针。`src/platform.rs` 的类型/契约拆出，`fonts.rs` 用系统文字栈替代。测试映射完成前不删除原测试。共享 app 的 `Cargo.toml`、`src/lib.rs` 随全仓清理删除。

`crates/petsona-shell-macos/src/platform.rs`、`autostart.rs` 的行为/几何测试迁至 Swift；macOS 新入口完整通过后删除其 `main.rs`、`lib.rs`、`Cargo.toml` 和剩余实现。Windows 旧 shell 的 `platform.rs`、`menu.rs`、`no_activate.rs`、`autostart.rs`、`main.rs`、`lib.rs`、`build.rs`、`Cargo.toml` 留到 Windows 对等验收完成。

### Windows 后续清单（当前不执行）

保留原计划的最终去向，接手 Windows 前需在对应环境核对版本/依赖并建立具体执行批次：

- `apps/windows/Petsona.sln`、`global.json`、`Directory.Build.props`、`.editorconfig`：工程/SDK/分析器/格式。
- `Petsona/Petsona.csproj`、`packages.lock.json`、`app.manifest`、`App.xaml`、`App.xaml.cs`：WinUI 入口、DPI、原生依赖和生命周期。
- `Petsona.Core/Petsona.Core.csproj`、`NativeMethods.cs`、`EngineHandle.cs`、`EngineClient.cs`、`AppState.cs`：同一 ABI 的 LibraryImport、生命周期/事件/快照。
- `Petsona/Native/WindowCoordinator.cs`、`PetWindow.cs`、`OverlayWindow.cs`、`ComposerWindow.cs`：窗口、原生 RichEdit 输入/IME、焦点与浮层。
- `Petsona/Native/PointerService.cs`、`MonitorService.cs`、`TrayService.cs`、`MenuService.cs`、`SystemServices.cs`：输入/掩码、DPI、托盘、菜单、文件/凭据/HKCU。
- `Petsona/Rendering/SpriteRenderer.cs`、`FrameScheduler.cs`：DirectComposition/Direct2D、资源缓存和设备恢复、低频调度。
- `Petsona/Views/SettingsWindow.xaml`、`.xaml.cs`、`PetLibraryPage.xaml`、`.xaml.cs` 和 `ViewModels/SettingsViewModel.cs`、`PetLibraryViewModel.cs`：原生页面与共享命令绑定。
- `Petsona/Diagnostics/TestProbe.cs`、`Petsona.Tests/Petsona.Tests.csproj`、`AbiTests.cs`、`EngineClientTests.cs`、`CoordinateTests.cs`、`StateProjectionTests.cs`：测试专用探针和真实跨语言验证。
- `scripts/verify-windows.ps1`、`windows-smoke.ps1`、`package-windows.ps1`、`.github/workflows/release-windows.yml`：Rust+.NET+原生测试、完整运行依赖包与 Release。
- `packaging/windows/Petsona.rc` 在 .NET 接管图标/版本后删除；`generate-icon.ps1` 保留。两端通过才清理旧 UI 依赖。

## 7. 必须保持的约束和测试命令

### 约束

1. 数据目录 `%APPDATA%/Petsona`、`~/Library/Application Support/Petsona` 和 `PETSONA_HOME` 不变；旧 schema/默认值保持，必要格式变化须用户确认迁移方案。
2. 无内置发行宠物；只读 Codex 导入源。保留自绘夹具，不使用真实用户宠物作自动测试输入。
3. 协议仅 loopback，默认 17872，TTL=0 不过期；凭据服务/账户 `com.petsona.desktop` / `deepseek`、环境变量优先级保留，密钥不进日志/快照。
4. 宠物/影子/气泡不激活，设置/输入框可激活；点菜单外不引发额外挥手；平台前端不能复制共享状态机。
5. 位置用物理像素；AppKit 坐标必须逐屏换算；Windows 旧入口相关契约保持。
6. 原生输入 Enter 发送、Shift+Enter 换行、Esc 关闭，IME 组合期间 Enter 只提交候选。关闭保留草稿。
7. 图集按代次传资源，不逐帧跨 ABI 复制整图；高频输入/帧用类型化数据；网络/磁盘不阻塞 UI。
8. panic 不跨 ABI，故障后拒绝业务命令；stop 后无晚到回调；原生 UI 只在主线程使用。
9. 测试隔离整个宿主；正式包不能启用 test-hooks；不以删除失败测试或 SKIP 达到全绿。
10. Git 默认只读，不自动提交/推送/打 tag；不新增包装脚本。平台实机结果必须写明机器/系统/架构。

### 自动化命令

从仓库根运行。这是目标门禁，不代表当前已通过；涉及尚未实现的 target/feature 须先实现再运行。命令失败按退出码记录；如通过管道过滤日志必须使用 `pipefail` 或显式保留原命令退出码。

T-R（共享 Rust，已有入口）：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

T-F（FFI，须补足上述测试；feature 本身存在不代表探针已实现）：

```bash
cargo test -p petsona-ffi --locked
cargo test -p petsona-ffi --release --locked
cargo clippy --workspace --all-targets --features petsona-ffi/test-hooks --locked -- -D warnings
cargo test --workspace --features petsona-ffi/test-hooks --locked
clang -fsyntax-only -x c contracts/petsona.h
```

头文件语法通过不能替代精确布局/生成一致性验证；后两者必须集成 T-F/T-S。

T-B（当前 Mac arm64 示例，真正跨架构须先构建匹配 Rust target，不能只改 `-arch`）：

```bash
xcodegen generate --spec apps/macos/project.yml --project apps/macos
xcodebuild -project apps/macos/Petsona.xcodeproj -scheme Petsona -configuration Release -arch arm64 -derivedDataPath .scratch/native-release CODE_SIGNING_ALLOWED=NO build
```

T-S（先修测试宿主隔离；当前有单测 target，UI target 尚待增加）：

```bash
xcodebuild -project apps/macos/Petsona.xcodeproj -scheme Petsona -configuration Debug -destination 'platform=macOS,arch=arm64' -derivedDataPath .scratch/native-tests -only-testing:PetsonaTests CODE_SIGNING_ALLOWED=NO test
xcodebuild -project apps/macos/Petsona.xcodeproj -scheme Petsona -configuration Debug -destination 'platform=macOS,arch=arm64' -derivedDataPath .scratch/native-ui-tests -only-testing:PetsonaUITests test
```

必须记录 xcresult 摘要/执行测试数量，不仅是退出码。测试进程的数据目录与端口由测试 harness 在入口前创建并注入，不能使用固定共享测试 home。

T-N/T-G（脚本必须先改为验证新原生入口，当前完整脚本仍测旧 shell）：

```bash
bash scripts/verify-macos-all.sh --gates-only
bash scripts/verify-macos-all.sh
```

T-P（打包脚本当前仍产旧入口，先修正再运行）：

```bash
bash scripts/package-macos.sh
otool -L dist/Petsona.app/Contents/MacOS/Petsona
lipo -archs dist/Petsona.app/Contents/MacOS/Petsona
```

检查包内所有 Mach-O，不得有仓库绝对依赖或遗漏动态库；复制到新的临时目录，隔离用户数据运行 `/health` 和窗口 smoke，并在无开发工具的 Mac 验证。签名/公证沿用既有脚本，由真实凭据驱动，缺凭据保持未通过。发布工作流改好后不自动触发 tag/外部发布。

Windows 后续：`cargo fmt/clippy/test` + `dotnet restore --locked-mode`、`dotnet format --verify-no-changes --no-restore`、`dotnet build/test -c Release -p:Platform=x64`，各自指向未来 solution/测试工程；最终统一 `powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1 -Full`。当前无 Windows 工程，禁止记为通过。

### 人工验收

| 编号 | 标准 |
|---|---|
| M-01 | 空库自动设置→导入→显示→切宠/导出/删除；人格、记忆、协议、DeepSeek 设置完整；数据重启保持 |
| M-02 | 注视/单双击/拖动/菜单/穿透，无白线/闪框/意外淡入；编辑其他应用时宠物不抢焦点 |
| M-03 | 0.5–2.0 缩放锚点/比例正确；Retina+外屏/负坐标/拔屏/重启正确；重力工作区落地一次跳跃；自动/立即活动正确 |
| M-04 | 至少一种中文 IME；候选确认不误发送；选区/粘贴/撤销/草稿/快捷键/caret 注视/输入框进退动画正确 |
| M-05 | 登录自启、Keychain、单实例、退出端口释放、干净安装、签名/公证与目标机器运行 |
| M-06 | 输入框开关100次、切宠50次、隐藏恢复50次，无崩溃/死锁/持续资源增长；预热后测5分钟，记录机器/宠物/CPU口径，复验约0–1%空闲目标及无空转60FPS |

## 8. 暂停与完成

遇到必须改变范围/契约/数据兼容/技术栈的冲突，写执行记录 CR，列证据、影响、可选处理和用户决定；不得自行改计划。当前已知 REV 问题修复属于原计划范围，不能把实现缺陷当作用户必须重新授权的理由。缺环境时先完成可安全验证的工作，相关必需验收保持受阻。

当前 macOS 任务完成 = 所有 MAC/SHARED REQ 实现与验收有证据、REV-01–09 已关闭、无新未决阻塞，发布包确为原生且可独立运行。签名/人工项未完成则明确“代码完成，验收未完成”，不得宣称整体完成。

全项目完成另外要求 REQ-16：Windows 对等验收与旧 UI/外壳清理。不得混淆两个完成口径。

## 9. 修订记录

| 版本 | 依据 | 改变 |
|---|---|---|
| 1.0 | 用户确认原生路线、授权先 macOS，随后要求按跨对话文档建议整理 | 恢复原完整计划、逐文件表和验收；加入可追踪编号；现有实现偏差留在执行记录，不降格为骨架目标 |

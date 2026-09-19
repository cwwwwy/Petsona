# native-ui-rewrite 执行与审查记录

## 任务身份

- 契约：[计划 v1.0](../plans/native-ui-rewrite.md)；项目规则：[AGENTS.md](../../AGENTS.md)。
- 当前状态：**实施中，原生验收不通过**。
- 当前范围：macOS + 必要共享层；Windows 延后。现有实现是骨架，不是用户批准缩减的交付目标。
- 基线：HEAD `ad045bb` / main，存在未提交原生代码；本记录整理于 2026-09-19。
- 本轮执行授权：用户于 2026-09-19 明确要求“按计划实施当前 macOS 与必要共享层范围”，允许修改产品代码与本执行记录；Git 仍遵守 AGENTS.md，只做只读查询，不代用户 add/commit/push。

## 0. 本轮接手与执行边界（2026-09-19）

- 接手 HEAD：`ad045bb` / `main`；工作区已有用户/上一执行者未提交改动，未做 reset、checkout、stash 或覆盖。
- 接手 dirty 文件：`AGENTS.md`、`Cargo.toml`、`Cargo.lock`、`README.md`、`crates/petsona-runtime/src/logging.rs`、`docs/PLATFORM_ARCHITECTURE.md`、`docs/MACOS_VERIFICATION.md`、`scripts/verify-macos-all.sh`；未跟踪 `apps/macos/`、`contracts/`、`crates/petsona-ffi/`、计划/执行/功能表文档。
- 目标复述：实现当前计划的 REQ-01～REQ-15、REQ-17；REQ-16 Windows 原生前端后置。旧 egui / 旧 shell 保留为对照，不能作为新入口的验收证据。
- 验收底线：每个 REQ 分别记录实现、自动测试、人工验收；统一 macOS 脚本必须实际构建/启动/检查新 `.app`；静态链接、测试隔离、生命周期/故障和完整功能不能以骨架或旧入口结果替代。
- 发现状态：未发现需要新增 CR 的范围/技术栈/数据契约冲突；现有 REV-01～REV-09 属于计划内缺陷修复，本轮继续处理。若后续必须改变 ABI、兼容格式或平台范围，将先在“冲突与变更请求”中暂停。

## 1. 已有改动清单（存在 ≠ 验收通过）

| 路径 | 实际改动 | 状态与限制 |
|---|---|---|
| `Cargo.toml` / `Cargo.lock` | 新 ffi 成员，release panic=unwind | 有代码；终止状态未闭环，REV-05 |
| `crates/petsona-ffi/src/lib.rs` / `Cargo.toml` | 同步创建/销毁、tick、快照、文本复制、6类命令、3个单测 | 雏形；业务侵入 FFI、TTL 和故障处理缺失 |
| `crates/petsona-runtime/src/logging.rs` | init 改 try_init | 防重复 panic；不能据此证明多实例日志隔离 |
| `contracts/petsona.h` / `ABI.md` | 手写头和线程/所有权描述 | 尚无头生成/精确跨语言布局检查；无已提交记录 |
| `apps/macos/project.yml` / `Petsona.xcodeproj` | XcodeGen、SwiftUI app、宿主 XCTest target | 有构建产物；实际链接动态库绝对路径，REV-01；宿主未隔离，REV-09 |
| `apps/macos/Petsona/Sources` | 入口、EngineClient、设置骨架、菜单、PetWindowController | 无完整宠物库/输入框/注视等功能；不能替代旧产品 |
| `apps/macos/PetsonaTests/EngineClientTests.swift` | 一个空库快照测试 | 无真实宠物、命令、释放、窗口或 IME 覆盖 |
| `scripts/verify-macos-all.sh` | 增加原生 Release build 和架构参数 | 未接入 XCTest；smoke/打包仍针对旧 shell |
| AGENTS、README、架构/验收/计划文档 | 增加原生方向说明 | 原完成声明已在本轮纠正，旧入口记录按历史保留 |

## 2. REQ 状态

实现/自动验证/人工验收分别记录；未执行不写通过，旧测试不覆盖新链路。

| REQ | 实现状态 | 自动验证 | 人工验收 | 当前缺口 |
|---|---|---|---|---|
| 01 | 偏离契约 | 无新运行时边界验证 | 不适用 | FFI 直接改 runtime，同步 UI IO，REV-02 |
| 02 | 部分 | 3个基础 Rust 测试、1个 Swift 空库测试的历史结果 | 不适用 | 精确布局/故障/销毁/事件/取消缺失，REV-05/08 |
| 03 | 不通过 | otool 已确认反例 E-06 | 未做 | 动态库绝对路径依赖，REV-01 |
| 04 | 部分复用旧代码 | 旧模型测试成功不代表新入口兼容 | 未做 | 旧数据/凭据/保存失败链路未覆盖 |
| 05 | 未完成 | 无新宠物库测试 | 未做 | 无原生导入/导出/覆盖/删除/切换入口 |
| 06 | 不通过 | 静态审查反例 REV-07 | 未做 | 空库不自动开设置，提示导入但无入口 |
| 07 | 部分且有回归 | TTL 单测只测旧核心，REV-03 | 未做 | 新 tick 未执行 TTL；注视输入未迁移 |
| 08 | 不通过 | 静态审查反例 REV-04/07 | 未做 | 整窗穿透，按下即挥手，无单双击/拖动判定 |
| 09 | 未完成 | 无原生几何/活动测试 | 未做 | 缩放未应用，活动误为无限 running，位置/重力未迁移 |
| 10 | 未实现完整 UI | 无 | 未做 | 气泡/影子/输入框/IME/caret 路径未完成 |
| 11 | 未完成 | 无新业务链路验证 | 未做 | 设置骨架，没有完整人格/记忆/对话 |
| 12 | 部分复用 | 旧协议/锁测试历史通过 | 未做 | 新入口 TTL/单实例失败处理，Keychain/自启接入待验 |
| 13 | 不通过 | 静态审查反例 REV-05/06 | 未做 | 空库/错误16ms循环，fault/stop未闭环 |
| 14 | 不通过 | REV-09 宿主隔离反例 | 未做 | 默认用户引擎仍启动；Rust测试仍用默认协议端口 |
| 15 | 未完成 | 原生 build历史通过，旧 smoke/包不作原生证据 | 未做 | 原生测试门禁、打包、签名/公证未闭环 |
| 16 | 后续范围 | 未执行 Windows 原生验证 | 未做 | 无 Windows 原生工程，旧入口保留 |
| 17 | 文档已整理 | 本轮链接/编号/非文档改动检查 | 不适用 | 待执行对话补每项测试和人工证据 |

## 3. 历史命令证据

以下转录自上一轮工具输出，不是本轮重跑。执行时 HEAD 为 ad045bb，工作区处于编辑中，没有保存逐次代码哈希；所以不能作为后续改动的最终验收。缺日志、退出码、测试数量时明确标注缺失。

| ID | 命令/目标 | 已知结果 | 证据位置与限度 |
|---|---|---|---|
| E-01 | `cargo fmt --all -- --check`；workspace | 历史 exit 0 | 原执行对话工具输出；只验证格式 |
| E-02 | `cargo clippy --workspace --all-targets --locked -- -D warnings` | 历史 exit 0 | 原执行对话；不能证明运行行为 |
| E-03 | `cargo test --workspace --locked` | 沙箱内4个协议绑定测试失败；升级权限后 exit 0，app25/core54/ffi3/runtime1/mac-shell7 | 原执行对话；有默认17872占用警告，测试不完全隔离；主要为旧入口测试 |
| E-04 | `cargo build -p petsona-ffi --release --locked`；`clang -fsyntax-only -x c contracts/petsona.h` | 历史 exit 0 | 编译与头语法；无跨语言精确布局证明 |
| E-05 | `xcodebuild` Debug/Release，arm64，新 app | 历史构建成功；曾遇沙箱宏插件、Swift编译、架构链接和Release测试target错误后调整 | 新 Release 位于 `.scratch/petsona-macos-release-arm64/Build/Products/Release/Petsona.app`；构建成功不代表静态链接/部署成功 |
| E-06 | `otool -L .scratch/petsona-macos-release-arm64/Build/Products/Release/Petsona.app/Contents/MacOS/Petsona` | 只读审查确认依赖 `/Users/book/Desktop/Petsona/target/release/deps/libpetsona_ffi.dylib` | 上一审查对话工具输出；与静态链接承诺冲突 |
| E-07 | `xcodebuild -quiet -project apps/macos/Petsona.xcodeproj -scheme Petsona -configuration Debug -arch arm64 -derivedDataPath .scratch/petsona-macos-test-final CODE_SIGNING_ALLOWED=NO test` | 历史包装命令输出 `xcodebuild_test_exit=0` | `.scratch/petsona-macos-test-final/Logs/Test/Test-Petsona-2026.09.19_17-31-05-+0800.xcresult`；执行测试数量/摘要未独立验证，源代码只有一个空库测试 |
| E-08 | `PETSONA_NATIVE_ARCH=arm64 bash scripts/verify-macos-all.sh --gates-only` | 输出截在 workspace release 构建；未留脚本最终退出码 | 不得记录统一脚本全部通过；尚未覆盖新原生 smoke/包 |
| E-09 | `git diff --check` | 历史 exit 0 | 只检查已跟踪 diff 空白，不覆盖未跟踪代码行为 |

E-07 的 xcresult 摘要在上一只读审查中因 TestReport 临时写入权限失败，不能补写“已核验测试数量”。多次较早的 test 调用丢失 exec session ID，结果包未完整收尾；不计为成功证据。后续执行必须保留 session_id，使用 write_stdin 等待真实退出，不反复启动重叠测试。

## 4. 审查发现（全部未修复）

| ID / 优先级 | REQ | 证据位置 | 问题及关闭所需证据 |
|---|---|---|---|
| REV-01 P1 | 03/15 | `apps/macos/project.yml:25`，E-06 | -l 选择 dylib，app依赖仓库路径；必须明确 .a、匹配架构，并以依赖检查+干净目录启动关闭 |
| REV-02 P1 | 01 | `crates/petsona-ffi/src/lib.rs:312,458`；EngineClient @MainActor | FFI加载文件/图集、直接改字段和保存，缺应用执行器；需runtime服务/后台IO以及线程/慢IO测试 |
| REV-03 P1 | 07/12 | `crates/petsona-ffi/src/lib.rs:357` | 新 tick 不调 PetEngine::tick，循环状态TTL不失效；需真实新入口协议→tick→回退集成测试 |
| REV-04 P1 | 08 | `apps/macos/Petsona/Sources/PetWindowController.swift:31` | 像素穿透等同整窗忽略鼠标，默认实体也不可点击；需原生掩码/idle回退测试 |
| REV-05 P1 | 02/13 | `crates/petsona-ffi/src/lib.rs:142`；`EngineClient.swift:40` | panic只返回码、stopped未置位，前端继续调度；需故障终止/命令拒绝/安全释放测试 |
| REV-06 P2 | 13 | `apps/macos/Petsona/Sources/AppDelegate.swift:41` | 无宠物或错误返回0后钳为16ms，约62.5Hz唤醒；需空库/失败/隐藏调度计数测试 |
| REV-07 P1 | 05–11/17 | AppDelegate:13/76、SettingsView:10、PetWindowController:70/73 | 空库不弹设置、无导入、缩放不应用、立即活动无限running、鼠标按下即挥手及大量功能缺失；需完整功能表逐项验收 |
| REV-08 P1 | 02/14/15 | `scripts/verify-macos-all.sh:109`、`scripts/package-macos.sh:32` | 门禁只build新app，后续测/打包旧shell；只有3+1基础测试，ABI大小只测下限；需新入口测试/布局/正式包验证 |
| REV-09 P1 | 14 | `apps/macos/Petsona/Sources/AppDelegate.swift:6`、scheme TEST_HOST | XCTest宿主先创建真实默认目录引擎，测试方法临时home无法隔离；需启动前宿主隔离并验证无真实用户目录/端口/凭据变更 |

关闭发现必须追加：修复文件/工作区版本、对应测试命令/退出码/报告、审查确认。不能仅把“已修复”写入状态列。上述行号是审查时定位，后续以符号和 diff 为准。

## 5. 偏差与决定

| ID | 偏差/决定 | 当前处理 |
|---|---|---|
| DEC-01 | 用户明确不要求Rust-only，选原生前端 | 已确认；保留Rust业务核心，平台UI可用C#/Swift |
| DEC-02 | 用户授权先macOS环境开发 | 已确认；Windows后续，并非将macOS范围缩为骨架 |
| DEV-01 | 执行将完整runtime/FFI架构改为同步主线程调用 | 未获用户批准的实现偏差；按REQ-01/02修正，不追认计划改变 |
| DEV-02 | 文档曾写“已提交”“稳定ABI/静态链接” | 本轮纠正文档；实际未提交，静态链接/生命周期仍未通过 |

如必须改变契约，新增 CR，写出事实、影响、建议、待用户选择；用户未决定前保留“待确认”，不修改计划目标。

## 6. 下一个执行对话

先读取计划v1.0及本记录，核对工作区，按REQ修复已知架构/测试隔离问题，再补macOS功能。不要重建另一套骨架，不删除仍需供Windows使用的旧共享UI。实施依赖以计划为准，所有必要验收完成前不宣布完成。

当前明确未做：上述REV修复、完整native smoke、真实IME/焦点/多屏/CPU验收、新包独立运行、签名公证、Windows前端。它们不会因本轮文档整理而变成已完成。

## 7. 后续证据填写格式

每次执行追加一条：日期/执行对话、计划版本、HEAD和工作区标识、REQ、改动文件、完整命令和cwd、OS/架构/目标入口、退出码、报告路径/测试数、失败或SKIP原因、人工证据、剩余项。历史失败不要删除；后续成功记录其替代关系。

## 8. 本轮执行增量（2026-09-19，持续追加）

### 8.1 共享 worker / FFI 边界

- 新增 `crates/petsona-runtime/src/commands.rs`、`events.rs`、`snapshot.rs`、`engine.rs`：runtime worker 在独立线程加载路径、配置、实例锁、宠物库和状态服务；UI 线程只发送命令并读取 `RuntimeSnapshot` / `RuntimeTexts`。无宠物时下一次 deadline 为秒级，不再由 FFI 把空库钳成 16ms；worker 在 tick 中执行 `PetEngine::tick`，所以协议 TTL 会真实回退。
- `crates/petsona-ffi/src/{types,buffers,commands,error,handles,render}.rs` 加入；`lib.rs` 改为薄 ABI 转换层，ABI 版本升为 2，增加 `ready/faulted/state_server_port` 和显式命令保留位；panic 后句柄进入 terminal fault，运行时失败/停止不再继续接受调用。
- `contracts/petsona.h` 同步 ABI 2；`apps/macos/project.yml` 改为直接链接 `target/release/libpetsona_ffi.a`，并用 XcodeGen 重新生成 `Petsona.xcodeproj`。
- 新增 runtime/FFI 隔离 worker、空库 deadline、ABI layout、无效命令测试。当前尚未接入完整原生功能，不能关闭所有 REV。

### 8.2 当前命令证据

| 证据ID | REQ/REV | cwd/目标 | 命令 | 结果 | 限制 |
|---|---|---|---|---|---|
| E-10 | REV-02/03/05，macOS arm64 Rust | `/Users/book/Desktop/Petsona` | `cargo fmt --all && cargo test -p petsona-ffi -p petsona-runtime --offline` | exit 0；FFI 3、runtime 3 全部通过 | 协议绑定未在该单测中启用；完整 workspace 尚未重跑 |
| E-11 | REV-01/15，macOS arm64 原生 Release | `/Users/book/Desktop/Petsona` | `cargo build -p petsona-ffi --release --offline && xcodegen generate --spec apps/macos/project.yml --project apps/macos` | exit 0；Rust release 与 XcodeGen 成功 | 仅生成/编译前置，不代表 app 已链接成功 |
| E-12 | REV-01/15，macOS arm64 原生 Release | `/Users/book/Desktop/Petsona` | `xcodebuild -project apps/macos/Petsona.xcodeproj -scheme Petsona -configuration Release -arch arm64 -derivedDataPath .scratch/native-release-current CODE_SIGNING_ALLOWED=NO build` | exit 65 | SwiftUI macro plugin 在受限会话被 `sandbox-exec` 拒绝；另有 ABI 结构新增 `reserved` 后 Swift 初始化器未同步，已修复；需升级权限重跑 |

### 8.3 当前状态变更

- REV-02：**部分修复**；FFI 不再直接加载/保存/修改 runtime，慢 IO 下沉到 worker；完整后台任务取消/网络请求/功能命令仍未完成。
- REV-03：**部分修复**；worker tick 已执行 `PetEngine::tick`；新入口协议→worker→快照→原生显示的集成测试尚未补齐。
- REV-05：**部分修复**；FFI panic/worker fault 有终止状态；跨语言销毁/故障后的 Swift 调度拒绝测试尚未补齐。
- REV-01/08/09：**未关闭**；静态链接已改向 `.a`，但尚未取得成功 app 与 `otool` 证据；XCTest 宿主隔离和统一脚本切换仍未完成。

### 8.6 最新补充（2026-09-19）

- 原生新增 `SystemServices.swift`：LaunchAgent 使用 `~/Library/LaunchAgents/com.petsona.desktop.plist`，测试可用 `PETSONA_AUTOSTART_PLIST_DIR` 隔离；Keychain 保存入口使用 `com.petsona.desktop/deepseek`；菜单现在列本地宠物并标记当前项；设置提供导入、导出、删除确认、人格、行为、自启和密钥入口。
- 原生 XCTest 显式测试类命令最终通过 4/4：`EngineClientTests` 2、`AbiTests` 1、`SystemServiceTests` 1；最新全量摘要位于 `.scratch/macos-native-tests/test-summary.json`。
- E-19：`bash scripts/verify-macos-all.sh` 在 MacBook Air arm64 / macOS 27 最终 exit 0；Rust workspace（app25/core54/ffi3/runtime3/mac shell7）、native XCTest 4/4、native smoke 6/6、静态依赖/架构、bundle/zip、LaunchAgent 模板均通过。
- `verify-macos-all.sh` 的 native XCTest 改为显式列出三类测试，避免当前 Xcode 对 target 级 `-only-testing:PetsonaTests` 的发现结果与实际测试类不一致；没有降低测试范围。
- 最后增量：AppDelegate 按宠物附近椭圆区域轮询 AppKit 全局光标，Composer 打开时使用 caret 目标，worker 对已有 gaze 使用 `retarget_gaze`；立即活动命令改为 8 秒 TTL，避免 native 菜单留下无限 `running`。
- 2026-09-19 人工验收便利化：设置新增“从 Codex 导入…”按钮，直接把文件面板定位到 `~/.codex/pets`，仍需用户选择具体宠物目录；打包脚本已生成当前未签名包 `dist/Petsona.app` 与 `dist/Petsona-macos-arm64.zip`，包内检查无 `pet.json`/默认宠物，应用空库启动后进入手动导入设置。
- 用户新一轮人工反馈后新增修复：Codex 改为候选扫描/预览/逐项导入；双击不再打开 Composer，恢复跳跃反馈；输入入口改为宠物下方编辑按钮；气泡点击打开 Composer、移除“回复”按钮并加入进场/退场动画；拖动状态节流、缩放窗口加入 AppKit 过渡；worker retarget/global gaze 接线保留。最终 dist 包已重建，native smoke 6/6 通过；视觉、跟手和注视仍待用户复验。
- 2026-09-20 针对人工复测反馈继续修复：全局注视增加“实际鼠标移动”门槛，静止光标不重新播放左向帧列；BubblePanel 避免每帧打断进场动画；拖动改用屏幕坐标而非移动窗口后的局部坐标，并节流 running 命令；缩放按当前窗口底部中心动画变更 frame；Composer 失去焦点后恢复编辑按钮；Codex 候选扫描加入预览列表。最新 `dist/Petsona.app` / arm64 zip 已重建，Rust Clippy、原生 Debug 构建和 native smoke 6/6 通过，仍需用户复测视觉结果。
- 2026-09-20 后续修正：按用户要求去除气泡进场动画；双击保持跳跃而不打开 Composer；拖动释放立即发 idle；静止光标不再发送 retarget；宠物 atlas 改用 AppKit source rect 裁剪，Codex 预览改为首帧裁剪而非整张图集。最新包已重建，native smoke 6/6 通过。
- 2026-09-20 最新反馈修复：gaze active 轮询固定到 16ms、idle 轮询 33ms；Codex 预览 JSON 改用 spritesheet 路径而非宠物目录；拖动/缩放/preview 代码已通过 Rust Clippy、runtime/FFI tests、原生 Debug 构建，dist 包与 native smoke 6/6 已刷新。等待用户复测视觉与实时跟随。
- 2026-09-20 用户确认其余验收项通过，仅建议扩大 gaze 范围；进入半径由短边 25% 留白扩大为 40%，已进入状态退出留白扩大为 50%，Rust Clippy/原生 Debug/最终 dist 包构建通过。
- 2026-09-20 追加诊断修复：确认 gaze 来回跳变的根因是“静止且仍在范围内”分支误发 `ClearGaze`；现在静止时保持当前姿势，只有鼠标移动才 retarget，离开范围才返回。最新包/Clippy/Debug/native smoke 6/6 通过。

### 8.7 用户人工复验结论（2026-09-20）

- 用户确认本轮目标项均无问题：扩大后的 gaze 范围、静止注视稳定性、宠物下方注视不再跳帧、Codex 首帧预览、编辑按钮/气泡输入入口、拖动、缩放和最终包均通过人工复验。
- 本结论关闭本轮针对性人工反馈；不等同于 M-01～M-06、签名/公证、多屏/Retina/Spaces 和完整发布验收全部完成。
- 本轮 Git 操作：无。工作区改动保留给用户手动审查、暂存、提交和推送。

### 8.4 后续证据（2026-09-19）

| 证据ID | REQ/REV | cwd/目标 | 完整命令 | 退出码/测试数 | 结果与限制 |
|---|---|---|---|---|---|
| E-13 | REQ-02/03/07/12/13/14，macOS arm64 | `/Users/book/Desktop/Petsona` | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets --offline -- -D warnings` | exit 0 | 共享 worker/FFI 改动无 Clippy 诊断 |
| E-14 | REQ-02/03/07/12/13/14，macOS arm64 | `/Users/book/Desktop/Petsona` | `cargo test -p petsona-runtime -p petsona-ffi --offline` | exit 0；runtime 3、FFI 3 | worker readiness、空库慢 deadline、ABI layout、无效命令通过；协议端口隔离配置关闭 |
| E-15 | REQ-02/03/14/15，MacBook Air arm64 / macOS 27 | `/Users/book/Desktop/Petsona` | `xcodebuild -quiet -project apps/macos/Petsona.xcodeproj -scheme Petsona -configuration Debug -destination 'platform=macOS,arch=arm64' -derivedDataPath .scratch/native-tests-current -only-testing:PetsonaTests CODE_SIGNING_ALLOWED=NO test` | exit 0；3/3 | xcresult：`.scratch/native-tests-current/Logs/Test/Test-Petsona-2026.09.19_21-23-29-+0800.xcresult`；测试 home 与 stateServer 均隔离 |
| E-16 | REQ-03/15，MacBook Air arm64 / macOS 27 | `/Users/book/Desktop/Petsona` | `bash scripts/package-macos.sh .scratch/package-check` | exit 0 | 原生 SwiftUI/AppKit Release `.app`、icns、zip 生成 |
| E-17 | REQ-03/07/12/13/15，MacBook Air arm64 / macOS 27 | `/Users/book/Desktop/Petsona` | `PETSONA_NATIVE_APP=.scratch/package-check/Petsona.app PETSONA_SMOKE_STATE_PORT=17972 bash scripts/macos-smoke.sh` | exit 0；6/6 | 原生 app 进程、`/health`、`/pets`、POST 状态、TTL 回退、安全退出通过 |
| E-18 | REQ-01～03/07/12～15/17，MacBook Air arm64 / macOS 27 | `/Users/book/Desktop/Petsona` | `bash scripts/verify-macos-all.sh` | exit 0 | Rust 25 app/54 core/3 ffi/3 runtime/7 old mac shell 全绿；原生 XCTest 3/3；native smoke 6/6；包结构/静态依赖/架构通过。此处旧 shell 测试仅为 Windows/迁移期兼容回归，不作为 native UI 证据 |

### 8.5 REV 关闭/剩余

- REV-01：**关闭**。`project.yml` 与生成工程直接链接 `libpetsona_ffi.a`；E-16/E-18 的 `otool -L` 检查无仓库绝对路径、无 `libpetsona_ffi` 动态依赖，`lipo -archs=arm64`。
- REV-02：**部分关闭**。FFI 已是薄转换层，磁盘/宠物库/协议/DeepSeek 对话在 runtime worker；仍需把 atlas 字节/缓存从 Swift UI 磁盘读取迁出，并补取消/慢网络任务证据。
- REV-03：**关闭**。worker tick 调用 `PetEngine::tick`，E-17 通过新原生 app 的 POST 状态→health waiting→TTL idle。
- REV-04：**部分关闭**。原生 `PetView.hitTest` 使用当前帧 alpha + idle 行并集，整窗不再 `ignoresMouseEvents`；仍需 M-02 实机确认透明像素/触控板。
- REV-05：**部分关闭**。FFI panic/worker fault 进入 terminal 状态，命令拒绝和销毁重建已有基础覆盖；仍需显式 panic 注入与 Swift fault UI 测试。
- REV-06：**关闭**。native AppDelegate 使用 snapshot deadline；空库 worker deadline 秒级，E-10/E-14 和 E-17 通过；Activity Monitor 人工仍待。
- REV-07：**部分关闭**。原生入口已接空库自动设置、导入/切换、缩放、行为、人格、气泡/对话、单双击/拖动和协议；导出/删除/完整菜单/LaunchAgent/Keychain UI 仍待。
- REV-08：**部分关闭**。统一脚本现在实际构建/测试/启动/打包 native app，E-18 通过；原生 UI/XCTest 覆盖仍少于完整 M-01～M-06，签名/公证未做。
- REV-09：**关闭**。XCTest 入口不创建默认 `EngineClient` 宿主；每个测试先写临时 home 并关闭状态服务，E-15 3/3 通过。

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

### 8.8 清理记录（2026-09-20）

- 删除 `docs/NATIVE_REWRITE_PLAN.md`：其内容明确声明已被计划 v1.0 取代，且旧结论会误导当前状态。
- 删除 `packaging/macos/Info.plist`：没有工程或脚本引用；native bundle 唯一 plist 为 `apps/macos/Petsona/Info.plist`。
- 修正 `AGENTS.md`、`docs/MACOS_VERIFICATION.md`、`docs/PLATFORM_ARCHITECTURE.md` 中的旧计划引用。
- 删除根目录和 `dist/` 下生成的 `.DS_Store`；保留旧 Windows/egui 对照入口、签名/公证/LaunchAgent 脚本、模板、执行记录和当前 native 工程。

### 8.9 短期执行范围确认（2026-09-20）

- 用户确认本批次继续实施剩余功能 4–10：DeepSeek 完整设置、记忆管理、人格管理/偏好记忆、宠物重复导入覆盖确认、拖放导入、快捷菜单缩放档位。
- 用户明确暂缓功能 1/2/3/11/12/13/14：自动活动提醒、重力、native 状态协议设置界面、宠物图标托盘化、影子到编辑按钮动画、透明度设置、多屏/Retina/Spaces/工作区适配。
- 该范围过滤只影响短期执行顺序，不把正式计划中的后续 REQ 标记为完成；暂缓项保留在计划和功能表中。

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

### 8.10 短期功能批次实现与验证（2026-09-20）

本批次按用户确认的 4–10 执行，未恢复已暂缓的 1/2/3/11/12/13/14。

- 4 DeepSeek 完整设置：runtime 投影非敏感配置，支持 Base URL、模型、环境变量名、超时、最大 token、温度和思考模式；macOS 设置页保存配置，Keychain 支持保存、存在性显示和清除。密钥不进入 runtime snapshot。
- 5 记忆管理：新增当前人格的记忆配置、事实列表、近期事件展示、手工添加/删除事实和清空入口；projection 包含配置但不含凭据。
- 6 人格管理：新增人格列表、切换、新建模板、复制、删除、JSON 导入导出和全字段编辑；删除默认人格仍由 core 拒绝。
- 7 偏好提取：对话输入记录事件，并仅从明确的第一人称表达（如“我喜欢… / 我不喜欢… / 请叫我…”及对应英文表达）生成事实；事实进入后续 DeepSeek prompt 上下文，不做不确定推断。
- 8 重复导入确认：runtime 在复制本地文件前读取目录或 zip 的 `pet.json` 身份；同 ID 时发布待确认路径，macOS 设置页明确“覆盖导入/取消”，确认后才替换本地包。
- 9 拖放导入：设置页接受宠物目录或 zip 的 file URL drop，并复用同一导入/冲突确认流程。
- 10 固定缩放档位：移除连续 slider，统一使用 0.5/0.75/1.0/1.25/1.5/1.75/2.0；runtime 归一化任意调用值，设置页和状态栏快捷菜单共用同一档位语义。

共享/原生接线文件：`crates/petsona-core/src/memory.rs`、`crates/petsona-core/src/pet/library.rs`、`crates/petsona-runtime/src/{commands,engine,session,snapshot}.rs`、`crates/petsona-ffi/src/{commands,render,types}.rs`、`contracts/petsona.h`、`apps/macos/Petsona/Sources/{EngineClient,SettingsView,AppDelegate,SystemServices}.swift`、`apps/macos/PetsonaTests/EngineClientTests.swift`。功能表、ABI 说明和 macOS 验收清单同步更新。

| 证据ID | REQ/批次 | cwd/目标 | 完整命令 | 退出码/测试数 | 结果与限制 |
|---|---|---|---|---|---|
| E-20a | 4–10 / SHARED，macOS arm64 | `/Users/book/Desktop/Petsona` | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0 | workspace clippy 无诊断 |
| E-20b | 4–10 / SHARED，macOS arm64 | `/Users/book/Desktop/Petsona` | `cargo test --workspace --locked` | exit 0；app25/core56/ffi3/runtime4/mac-shell7 | 状态协议测试在允许回环端口的执行权限下通过；新增偏好、导入身份、runtime 设置投影测试通过 |
| E-20c | 4–10 / ABI，macOS arm64 | `/Users/book/Desktop/Petsona` | `clang -fsyntax-only -x c contracts/petsona.h`；`cargo test -p petsona-ffi -p petsona-runtime --offline` | exit 0；FFI3/runtime4 | 新文本字段与命令枚举保持 ABI 3 结构布局；不含密钥 |
| E-20d | 4–10 / macOS Debug | `/Users/book/Desktop/Petsona` | `xcodebuild -quiet -project apps/macos/Petsona.xcodeproj -scheme Petsona -configuration Debug -destination 'platform=macOS,arch=arm64' -derivedDataPath .scratch/native-settings-tests -only-testing:PetsonaTests/EngineClientTests -only-testing:PetsonaTests/AbiTests -only-testing:PetsonaTests/SystemServiceTests CODE_SIGNING_ALLOWED=NO test` | exit 0；5/5 | xcresult：`.scratch/native-settings-tests/Logs/Test/Test-Petsona-2026.09.20_01-54-25-+0800.xcresult`；MacBook Air arm64 / macOS 27 |
| E-20e | 4–10 / REQ-03/14/15，MacBook Air arm64 / macOS 27 | `/Users/book/Desktop/Petsona` | `bash scripts/verify-macos-all.sh` | exit 0；XCTest5/5、native smoke6/6 | 原生 Release、workspace Rust、协议/TTL、安全退出、静态依赖、arm64 bundle/zip、Info.plist 和 LaunchAgent 模板均通过；真实窗口/Keychain/拖放/菜单仍需人工 |
| E-20f | 4–10 / 交付包，MacBook Air arm64 / macOS 27 | `/Users/book/Desktop/Petsona` | `bash scripts/package-macos.sh`；`PETSONA_NATIVE_APP=dist/Petsona.app PETSONA_SMOKE_STATE_PORT=17972 PETSONA_SMOKE_HOOK_PORT=17973 bash scripts/macos-smoke.sh` | exit 0；dist smoke6/6 | 刷新 `/Users/book/Desktop/Petsona/dist/Petsona.app` 与 `dist/Petsona-macos-arm64.zip`；未签名，未内置 `pet.json`，最终包可启动并安全退出 |

本批次未执行 Git add/commit/push；工作区既有改动及本批次改动均保留给用户审查。人工验收按 `docs/MACOS_VERIFICATION.md` 的 A4/A6/A11、B8/B9 及 M-01/M-04 进行；多屏/Retina/Spaces、重力、自动活动、透明度、签名/公证和宠物图标托盘化保持暂缓或待凭据。

### 8.12 macOS 设置页侧边栏布局（2026-09-20）

- 用户要求设置菜单改为侧边栏布局。
- `SettingsView.swift` 改用 `NavigationSplitView`：左侧分为“宠物库 / 外观与交互 / DeepSeek / 人格 / 记忆 / 启动”，右侧显示对应原有设置内容；保留现有命令、表单绑定、导入导出和人格 sheet 行为。
- `AppDelegate.swift` 将设置窗口初始尺寸调整为 960×700、最小尺寸 820×600，适配侧边栏与详情页。
- 自动验证：`xcodebuild` Debug 编译 exit 0；`bash scripts/verify-macos-all.sh` exit 0；原生 XCTest 5/5；native smoke 6/6；`bash scripts/package-macos.sh` exit 0；最终 `dist/Petsona.app` smoke 6/6。
- 当前仍需人工确认：侧边栏选中态、窗口缩放/滚动、各分区表单操作，以及设置窗口重新打开后的选中分区体验。

### 8.13 macOS 设置焦点与宠物右键菜单修复（2026-09-20）

- 根因：侧边栏选择使用可选值绑定，交互反馈不稳定；设置窗口未显式声明可成为 Key/Main Window；宠物视图只有左键/拖动事件，没有 `rightMouseDown` 到原生菜单的接线。
- 修复：`SettingsView` 改为非可选 `SettingsSection` 选择绑定；新增可成为 Key/Main 的设置窗口并在打开时显式激活；`PetView` 增加右键事件，AppDelegate 弹出原生上下文菜单，包含设置、宠物选择、缩放、显示/隐藏、立即活动和退出。
- 自动验证：Debug build exit 0；`bash scripts/verify-macos-all.sh` exit 0；workspace app25/core57/ffi3/runtime5/mac-shell7；原生 XCTest 5/5；native smoke 6/6；最终 `dist/Petsona.app` smoke 6/6；`git diff --check` 通过。
- 人工待验：点击侧边栏切换六个分区、设置窗口焦点/按钮交互，以及真实鼠标和触控板右键菜单位置、点外关闭与 Esc 关闭。

### 8.14 macOS 对齐 Windows 阶段一：注视稳定器与拖动刷新（2026-09-20）

- 新增 `apps/macos/Petsona/Sources/GazeStabilizer.swift`，对齐 Windows 的官方 16 向方向量化、7° 迟滞、2px 最小移动和单位向量回环。
- `AppDelegate` 采用 80%/100% 进入/退出注视余量、35% 内死区，并在每个 gaze poll 重发稳定方向；拖动期间暂停注视。
- `PetWindowController` 增加拖动开始/移动/结束事件；AppDelegate 以 8ms 限流主动刷新 engine/window，降低 AppKit 事件跟踪模式下的拖动首帧停滞风险。
- 首次稳定器测试使用了已越过迟滞边界的输入，记录为失败后修正测试边界；实现未改变。该失败保留如下。

| 证据ID | 目标 | 命令 | 结果 |
|---|---|---|---|
| E-21a | 注视稳定器初次 XCTest | `xcodebuild ... -only-testing:PetsonaTests/GazeStabilizerTests ... test` | exit 65；`testHysteresisAndMinimumMovementHoldDirection` 的测试输入实际越过 22.5°+7° 边界，已修正为边界内输入 |
| E-21b | 注视/ABI/worker 原生测试 | `xcodebuild ... -only-testing:PetsonaTests/GazeStabilizerTests -only-testing:PetsonaTests/EngineClientTests -only-testing:PetsonaTests/AbiTests -only-testing:PetsonaTests/SystemServiceTests ... test` | exit 0；包含 GazeStabilizer 2 项，原生测试合计 7/7 |
| E-21c | Mac 完整门禁 | `bash scripts/verify-macos-all.sh` | exit 0；workspace app25/core57/ffi3/runtime5/mac-shell7；XCTest 7/7；native smoke 6/6；Release/静态依赖/arm64/包结构通过 |
| E-21d | 最终交付包 | `PETSONA_NATIVE_APP=dist/Petsona.app PETSONA_SMOKE_STATE_PORT=17972 PETSONA_SMOKE_HOOK_PORT=17973 bash scripts/macos-smoke.sh` | exit 0；dist smoke 6/6 |

本阶段仍需人工复验：光标停住时的跨行注视完成、迟滞边界不抖动、拖动时左右反向与连续帧，以及此前已通过的侧边栏和宠物右键菜单。未执行 Git add/commit/push。

### 8.11 范围决定：Windows 线并行启动（2026-09-20）

- 用户决定：Windows 原生线（C# / WinUI 3 + Win32）与 macOS 线**并行推进**；旧 Windows 线（egui/Win32 外壳）的 W-* 实机复测**冻结**。
- 本记录（macOS 线）状态不变：**代码完成、人工验收未完成**（REV-02/04/05/07/08 部分未关闭；M-01～M-06 人工项待做；签名/公证待凭据）。
- Windows 线契约：[windows-native-rewrite 计划](../plans/windows-native-rewrite.md) v1.0；执行记录：[windows-native-rewrite](../execution/windows-native-rewrite.md)。
- 并行规则（对两条线生效）：共享层/ABI/契约变更须双端回归（Rust 门禁 + macOS 原生门禁 + Windows 构建/测试）；`apps/macos/**` 仅允许共享层接口同步适配；任一端未回归前不得发布该端。

### 8.15 macOS 对齐 Windows 阶段二：资源与宠物库体验（2026-09-21）

- 本地宠物列表读取 runtime 已有的 `spritesheet`、`cellWidth`、`cellHeight` 投影，显示首帧缩略图，与 Windows 宠物列表预览路径对齐。
- 设置页缩略图新增主线程 `NSCache`，避免 engine tick 导致 SwiftUI 重绘时重复从磁盘读取同一张图。
- 新增 `PetResourceTests`，使用 `crates/petsona-core/testdata/v2-test-pet-webp/spritesheet.webp` 验证 AppKit `NSBitmapImageRep` 能读取 V2 WebP 图集。
- WebP 解码失败的 Release 根因是 Swift 6 将共享 `NSCache` 判为非并发安全；已将缓存限定到 `MainActor`，未改变资源数据或线程边界。

| 证据ID | 目标 | 命令 | 结果 |
|---|---|---|---|
| E-22a | 阶段二首次完整门禁 | `bash scripts/verify-macos-all.sh` | exit 65；Rust/Debug 路径通过，Release Swift 编译因 `NSCache` 缺少 `MainActor` 标注失败；失败日志保留在 `.scratch/phase2-verify.log` |
| E-22b | 修复后 Release | `xcodebuild -quiet -project apps/macos/Petsona.xcodeproj -scheme Petsona -configuration Release -arch arm64 -derivedDataPath .scratch/native-resource-release CODE_SIGNING_ALLOWED=NO build` | exit 0 |
| E-22c | WebP/注视原生测试 | `xcodebuild ... -only-testing:PetsonaTests/PetResourceTests -only-testing:PetsonaTests/GazeStabilizerTests ... test` | exit 0；目标测试通过 |
| E-22d | 阶段二原生测试摘要 | `xcodebuild ... -only-testing:PetsonaTests/EngineClientTests -only-testing:PetsonaTests/AbiTests -only-testing:PetsonaTests/GazeStabilizerTests -only-testing:PetsonaTests/PetResourceTests -only-testing:PetsonaTests/SystemServiceTests ... test` | exit 0；XCTest 8/8；xcresult：`.scratch/native-resource-tests-final/Logs/Test/Test-Petsona-2026.09.21_00-18-18-+0800.xcresult` |
| E-22e | 阶段二交付包 | `PETSONA_SKIP_BUILD=1 PETSONA_NATIVE_DERIVED_DATA=.scratch/macos-native-gates bash scripts/package-macos.sh`；`PETSONA_NATIVE_APP=dist/Petsona.app PETSONA_SMOKE_STATE_PORT=17972 PETSONA_SMOKE_HOOK_PORT=17973 bash scripts/macos-smoke.sh` | exit 0；dist smoke 6/6；arm64 包刷新 |
| E-22f | 阶段二最终完整门禁 | `bash scripts/verify-macos-all.sh > .scratch/phase2-final.log 2>&1` | exit 0；最终日志保留在 `.scratch/phase2-final.log`；XCTest 8/8、native smoke 6/6、Release/静态依赖/包结构全绿 |

本阶段仍需人工复验：设置页本地宠物缩略图观感、WebP 用户宠物显示、切换宠物后图集更新，以及阶段一注视/拖动行为在最新包中的稳定性。

### 8.16 macOS 对齐 Windows 阶段三：生命周期与原生 UI 契约测试（2026-09-21）

- `EngineClientTests` 新增同一隔离数据目录下的引擎销毁/重建和 DeepSeek 配置持久化测试。
- 新增 `NativeLifecycleTests`：验证设置侧边栏六个稳定分区，以及 BubblePanel 非激活、Composer 可激活的窗口焦点契约。
- 统一 macOS XCTest 纳入 EngineClient、ABI、GazeStabilizer、PetResource、NativeLifecycle、SystemService 六类测试，共 11 项。
- 本阶段仍不使用坐标驱动的脆弱 UI 自动化；右键、IME、窗口视觉和多次开关长测继续保留人工验收。

| 证据ID | 目标 | 命令 | 结果 |
|---|---|---|---|
| E-23a | 生命周期/UI 契约测试 | `xcodebuild ... -only-testing:PetsonaTests/EngineClientTests -only-testing:PetsonaTests/AbiTests -only-testing:PetsonaTests/GazeStabilizerTests -only-testing:PetsonaTests/PetResourceTests -only-testing:PetsonaTests/NativeLifecycleTests -only-testing:PetsonaTests/SystemServiceTests ... test` | exit 0；XCTest 11/11；xcresult：`.scratch/native-lifecycle-tests/Logs/Test/Test-Petsona-2026.09.21_00-33-48-+0800.xcresult` |
| E-23b | 阶段三完整门禁 | `bash scripts/verify-macos-all.sh > .scratch/phase3-final.log 2>&1` | exit 0；统一脚本显示 XCTest 11/11、native smoke 6/6、Release/静态依赖/包结构全绿 |
| E-23c | 阶段三交付包 | `PETSONA_SKIP_BUILD=1 PETSONA_NATIVE_DERIVED_DATA=.scratch/macos-native-gates bash scripts/package-macos.sh`；`PETSONA_NATIVE_APP=dist/Petsona.app PETSONA_SMOKE_STATE_PORT=17972 PETSONA_SMOKE_HOOK_PORT=17973 bash scripts/macos-smoke.sh` | exit 0；dist smoke 6/6 |

阶段三人工待验：设置分区切换和重开窗口、输入框/气泡焦点、IME、右键菜单、连续开关 100 次、第二实例、空闲 CPU 与资源释放。

### 8.17 macOS 对齐 Windows 阶段四：生命周期与第二实例门禁（2026-09-21）

- `AppDelegate` 在 runtime fault（包括数据目录锁冲突）时自动退出，避免第二实例留下无响应菜单栏进程。
- `EngineClientTests` 新增同一隔离目录下反复启动/销毁并验证 state server 端口与实例锁释放；`NativeLifecycleTests` 新增浮层反复创建/关闭测试。
- `macos-smoke.sh` 新增同一 `PETSONA_HOME` 第二实例自动退出检查，smoke 从 6 项扩展为 7 项。

| 证据ID | 目标 | 命令 | 结果 |
|---|---|---|---|
| E-24a | 第二实例初次包 smoke | `PETSONA_NATIVE_APP=dist/Petsona.app ... bash scripts/macos-smoke.sh` | exit 1；当时 dist 尚未刷新到本阶段 fault 自动退出实现，第二实例仍存活；失败日志已保留在本轮工具输出 |
| E-24b | 最新包第二实例 smoke | `bash scripts/package-macos.sh`；`PETSONA_NATIVE_APP=dist/Petsona.app ... bash scripts/macos-smoke.sh` | exit 0；7/7，第二实例自动退出通过 |
| E-24c | 阶段四完整门禁 | `bash scripts/verify-macos-all.sh > .scratch/phase4-final.log 2>&1` | exit 0；XCTest 13/13、native smoke 7/7、Release/静态依赖/包结构全绿 |

阶段四仍需人工确认：设置/Composer/气泡反复开关 100 次、第二实例真实桌面提示、Activity Monitor 5 分钟 CPU、退出后无残留窗口/端口/锁文件。

### 8.18 macOS 对齐 Windows 阶段五：发布前审计与交付包验证（2026-09-21）

- 发布工作流改为先运行完整 `verify-macos-all.sh`，再构建真实 `dist/Petsona.app`，最后对该 dist 包运行 `macos-smoke.sh`；artifact 名称明确带 `unsigned`，避免把未签名包误认为可发布包。
- 旧 `petsona-app`、旧 macOS/Windows shell 和相关 egui/winit 依赖完成只读引用审计后保留：REQ-16 要求 Windows 原生对等验收和两端发布包通过后才能清理，本阶段没有越过该前置条件删除文件。
- `actionlint` 在当前 Mac 未安装，未将其缺失伪装为 YAML 已通过；shell 语法、diff 空白和真实 Xcode/运行门禁替代验证已执行。

| 证据ID | 目标 | 命令 | 结果 |
|---|---|---|---|
| E-25a | 阶段五首次完整门禁 | `bash scripts/verify-macos-all.sh > .scratch/phase5-final.log 2>&1` | exit 101；受限执行环境禁止状态协议测试绑定 `127.0.0.1:0`，4 个 core 状态服务测试报 `Operation not permitted`；失败日志保留在 `.scratch/phase5-final.log` |
| E-25b | 发布链路语法/空白 | `bash -n scripts/verify-macos-all.sh scripts/macos-smoke.sh scripts/package-macos.sh scripts/sign-macos.sh scripts/notarize-macos.sh`；`git diff --check` | exit 0；`actionlint` 不可用，已明确记录为 SKIP |
| E-25c | 允许回环端口后的完整门禁 | `bash scripts/verify-macos-all.sh > .scratch/phase5-final.log 2>&1` | exit 0；原生 XCTest 13/13、native smoke 7/7、Release/静态依赖/arm64/包结构全绿；日志：`.scratch/phase5-final.log` |
| E-25d | 最终 dist 交付包 | `bash scripts/package-macos.sh dist`；`PETSONA_NATIVE_APP=dist/Petsona.app PETSONA_SMOKE_STATE_PORT=17972 PETSONA_SMOKE_HOOK_PORT=17973 bash scripts/macos-smoke.sh` | 两条命令均 exit 0；`dist/Petsona.app` 与 `dist/Petsona-macos-arm64.zip` 刷新，dist smoke 7/7；包仍明确为未签名 |

阶段五结论：发布链路代码和未签名交付包自动验证通过；真实 Developer ID 签名、公证、目标机器安装仍待凭据与人工验收。旧入口清理继续等待 REQ-16，不因本阶段自动门禁通过而提前删除。未执行 Git add/commit/push。

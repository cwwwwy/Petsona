# Petsona 项目须知（给 AI 协作者）

## 当前状态（核对于 2026-10-13）

- 当前版本0.1.0，进入M6发布准备。Windows/macOS主体功能人工验收与Windows发布环境回归均已通过；遗留专项见开发执行记录，不重复已通过清单。
- M6-A pre-release-cleanup已验收关闭（用户“两项通过”）：普通版设置/聊天、角色保存重开、默认日志通过；Windows验收版与普通版构建流程按用户实机确认，不补造退出码或报告。下一步M6-B安装包/签名公证，再发布流程、安装升级与干净机器验证；尚未生成正式安装包或发布。
- 自动证据分宿主记录：Mac27.0.1/arm64；Windows x86_64 PS5.1/MSVC当前验收此前已通过。旧报告按用户决定清理，不能据此补造新自动结果；其他硬件原生自动日志未提供，当前PASS不代表历史间歇原因已根治。
- 项目目录与 Git 历史收束已完成。长期计划和执行证据分别维护在 `docs/DEVELOPMENT_PLAN.md` 与 `docs/DEVELOPMENT_EXECUTION.md`；Git 写操作仍由用户执行。

## 架构与目录

Petsona是Windows/macOS桌宠与AI伴侣：Rust拥有业务、窗口与原生浮层，TypeScript只承载内容界面。

| 路径 | 职责 |
|---|---|
| crates/petsona-core | 宠物格式/动画/人格/记忆/模型/状态协议 |
| crates/petsona-runtime | 配置/会话/宠物库/锁/问候/网络/流式聊天/学习/日志 |
| apps/desktop/src-tauri | Tauri2平台层、托盘/窗口/凭据/自启、原生宠物/气泡/Composer |
| apps/desktop/src | React/TS设置六页、聊天/历史、角色、宠物库 |
| docs | 长期开发计划与执行记录；稳定架构/验收规则维护在本文件与README |

- WebView不渲染宠物/气泡/输入框。跨边界只传JSON命令/投影，不传Rust所有权或引用；没有C ABI。
- macOS最低支持版本为26.0（`apps/desktop/src-tauri/tauri.conf.json`）；Mac平台代码必须在Mac上实编实测。
- 当前仅Rust/TS产品线，不恢复旧C#/Swift/FFI前端。
- 根workspace只含core/runtime；桌壳独立Cargo工程，避免引入Linux WebKit门禁。
- Win原生浮层线程必须抽消息；Mac AppKit窗口/菜单/面板/浮层只能Tauri主线程，后台只处理runtime与解码。
- 几何唯一单位PhysicalRect物理像素；Windows夹取rcWork，Mac用visibleFrame，逻辑点只在边界转换。

## 协作硬规则

- 用中文。设置面向普通用户：角色名称与一段可见设定、自然语言记忆；内部ID/概率/协议/日志仅必要高级或排查入口。
- README.md 与 README.en.md 的用户可见信息必须同步维护；修改任一文件时同时核对并更新另一语言版本，保持产品状态、功能、构建命令和开发文档入口语义一致。
- Git只读status/log/diff/show/remote/branch查询。add/commit/push/tag/建删分支/改remote给用户命令，除非明确“你来操作”。每轮交付全部未提交改动分组和可直接执行的提交命令。
- 用户偏好集中简短人工清单，容易观察的项目不编写复杂控制脚本，不强制多份报告；失败才索取相关日志。需要人工时最终回复直接列命令、操作与预期，文档链接仅补充。
- 自动不移动鼠标、不注入键盘/鼠标/滚轮消息。点击/拖动/穿透/注视/悬停/托盘/IME一律人工；无输入门禁限启动/样式/几何/截图/焦点/单实例/协议/退出/空库。
- 验收隔离PETSONA_HOME、端口、凭据、自启值名/目录、日志；不启动默认数据实例。自动用假环境Key和本地假端点，不能改日常实例或全局杀同名进程。
- 减少第三方依赖，优先标准库和已有锁；Tauri2/Vite/React/TS生态已获准但版本锁定。
- 当前 `.github/workflows/ci.yml` 只在 main 的 PR、`windows-v*`/`macos-v*` 标签推送及手动触发时运行 Rust fmt/clippy/test；普通 main push 不触发。它尚未构建安装包、处理签名/公证或创建 GitHub Release。正式发布按平台区分；Git 写操作仍由用户执行。

## 计划与证据

- 新会话先读本文件、README.md/README.en.md、`docs/DEVELOPMENT_PLAN.md`、`docs/DEVELOPMENT_EXECUTION.md` 和 git log。
- `docs/DEVELOPMENT_PLAN.md` 是长期开发计划；未启动阶段可先记录目标和待决条件，进入实施前补齐范围、REQ、约束/依赖、验收矩阵、命令与完成条件。`docs/DEVELOPMENT_EXECUTION.md` 按阶段维护实现、自动验证、人工验收和遗留证据。默认更新这两份文档，不按阶段另建计划/执行文件，也不清理已完成阶段的历史证据。
- 开发阶段与验收状态以开发计划为准，实际结果以执行记录为准；AGENTS.md 只保留当前协作/技术规则和简要状态，阶段变化时核对更新。README.md 与 README.en.md 同步面向用户的发布状态。
- 规划调查只读；用户授权计划落盘或实施后再更新长期计划。执行前复述目标与验收标准。局部实现可自定，不缩减为骨架、不跳验收、不改变架构边界。
- 基线冲突、目标互斥、必须改契约/依赖/格式/平台范围或必要验收不可满足时，提供文件证据、影响和建议，暂停受影响工作。偏差先记执行记录，不能修改计划标准来适配现状；用户直接决定优先。
- 每个 REQ 分别记录实现/自动/人工状态；命令记录目标、架构、cwd/源状态、退出码和结果路径。当前阶段如实写 FAIL/SKIP；人工确认不改写自动 PASS，旧入口结果不替代当前版本。
- 所有必需项通过且无未解决审查项才宣布完成；人工未做写“待验收”，不重复已通过清单。

## 构建与验收

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --dir apps/desktop install --frozen-lockfile
pnpm --dir apps/desktop build
```

- 协议测试需要回环端口；沙箱拒绝bind属于环境失败，放行网络复跑且留记录。
- 验收构建显式acceptance-hooks；普通构建默认不含--verify-*/--open-composer/自动退出/截图实现。
- Mac：desktop-build-macos.sh [debug|release] [acceptance|formal]；验收版默认，输出.scratch/macos/{profile}；formal输出.scratch/macos/formal/{profile}。
- Windows：desktop-build-windows.ps1 -Profile debug/release -Mode acceptance/formal；验收版默认，输出desktop-target；formal输出desktop-formal-target。先pnpm build更新dist。PS5.1中文脚本需UTF8 BOM；Resolve-Path.ProviderPath用于原生路径。
- scripts只保留两端build与verify共4个入口。Windows verify对已构建的单一profile回归，不负责build；键鼠交互和视觉由用户手动验收。自动脚本不注入键鼠、不全局查杀同名进程。

## 数据与兼容事实

- 数据：%APPDATA%/Petsona或~/Library/Application Support/Petsona；PETSONA_HOME覆盖。只加载本地pets，Codex库仅导入源，无内置宠物。
- V2格固定192×208（1536×2288÷8×11），不读取包grid.cellWidth/Height；自绘夹具不是视觉/方向还原证据。webp一等格式，PNG/JPEG兼容。
- config.json：window.startPosition物理像素、缩放0.5–2.0七档；协议键stateServer.port（默认17872），不是蛇形键。
- 协议127.0.0.1端口，/health /pets /state，TTL/source/clear语义不变；日志logs/petsona.log与shell.log各4MiB+一备份，凭据仅系统库，锁petsona.lock。
- 默认Windows自启HKCU Run/Petsona；Mac LaunchAgent com.petsona.desktop。Mac只影响下次登录，不bootstrap/bootout；测试用专用label/directory/service。
- Windows登录验收launcher仅acceptance-hooks、Petsona.Accept.*值名、显式home及专用服务可用；普通构建拒绝验收launcher，自启仍直接exe。
- 角色characterDefinition可选，旧systemPrompt/traits/styleProfile完整投影，无启动覆盖迁移；稳定ID/宠物绑定保留，试聊不写历史/记忆。
- 记忆suppressed指纹阻止忘记/拒绝内容重学；纠正停止旧任务并推进游标；忘记记忆不删聊天历史，生成替换默认保留记忆。

## 实现陷阱

- Win窗口同步消息会重入，状态用try_borrow防御，重入交DefWindowProc，不能跨extern回调panic；跨线程GetWindowText会阻塞。
- 透明窗DWM边框设COLOR_NONE/圆角DONOTROUND，缓存比较尺寸/层级，别每帧SetWindowPos。窗口类设箭头与WM_SETCURSOR。
- HTTRANSPARENT跨线程不可靠，WS_EX_TRANSPARENT与闪烁取舍不能擅改。命中回退idle全帧并集，转向姿态透明处仍可点。
- V2注视是姿势表；椭圆触发区短边25%留白、退出35%，16ms步进，跨行经旧行边缘；离开回中性。Mac物理尺寸cell×scale，Retina边界除backing，sidecar保存显示器相对偏移/密度。
- 启动投影ready前文本可空：数组[]、对象{}归一化；前端只显示启动页，并保留模块/渲染失败兜底。
- accept socket显式恢复阻塞。Windows HKCU受限ACCESS_DENIED必须SKIP。PS传真正null用NullString；恢复原环境不能把不存在的变量变空串。
- WSL启动PS会污染PATHEXT；cmd不能UNC cwd，锚定用户本地路径再PushLocation。ZIP用ZipFile.CreateFromDirectory，避免PS5 Compress-Archive反斜杠问题。
- Mac代码在Mac实编/实测，Windows/Linux审查不算通过；其他架构/显示器原生证据不推测。

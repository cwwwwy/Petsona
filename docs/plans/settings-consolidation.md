# settings-consolidation：执行契约

> 本计划由 2026-09-21 对话中用户逐条确认后落盘（见 §身份与授权），执行者不得自行缩减范围。

## 身份与授权

- 任务ID / 版本 / 日期：`settings-consolidation` / `1.0` / 2026-09-21。
- 用户目标：收束设置项（删除与行为不符的设置）、按行业领先做法重做设置页 UI、补上空闲问候功能；Windows 与 macOS 同批。
- v1.1（2026-09-21 第二批决定）：见 §修订记录；Windows 侧第一批已实机通过（W15）。
- 已确认决定（2026-09-21 对话）：
  1. 卡片样式**手写、零新依赖**（不引入 Community Toolkit）；
  2. 接受**全部即时生效**，删除 DeepSeek / 人格 / 记忆三处「保存」按钮；
  3. 14 个死设置**完全移除，不保留**（UI + `persona.json` 字段 + `PersonaPatch` 字段）；
  4. **空闲问候补上**（runtime 定时器 + DeepSeek 调用 + 固定问候回退）；
  5. **两端同批**（Windows C#/WinUI 与 macOS SwiftUI 同一信息架构；macOS 需在 Mac 上回归）。
- 本次范围：`crates/petsona-core`（persona/config 数据模型）、`crates/petsona-runtime`（patch/问候调度）、`apps/windows/Petsona/Views`（设置页重做）、`apps/macos/Petsona/Sources/SettingsView.swift`（同构）、两端测试与验收文档。
- 非目标：新增主题开关、设置搜索、宠物图标托盘化、多屏 / 重力 / 活动提醒（仍按 windows-native-rewrite §3.1 暂缓）；不引入第三方 UI 包。
- HEAD / 分支：`main`，执行前以 `git log` / `git status` 核对；基线含未提交的 CR-W2 协议改动与文档归档（见 §命令证据）。
- 执行记录：`../execution/settings-consolidation.md`。

## 目标与需求

| REQ | 可验证要求 | 范围 | 完成标准 |
|---|---|---|---|
| REQ-S01 | 移除 14 个无行为字段：`persona.avatarPet`、`sampling.temperature`、`sampling.maxTokens`、`model.provider`、`model.model`、`memory.windowTurns`、`memory.longTerm`、`memory.summarizeAfterTurns`、`tts.enabled/voice/rate`、`proactive.enabled/idleMinutes`、`config.memory.retentionDays`；`PersonaPatch` 同步删除。旧 `persona.json` 仍能加载（serde 忽略未知字段） | 本次 | Rust 单测：旧 JSON 含未知字段可加载；`persona.json` 往返后不再写出这些字段 |
| REQ-S02 | 记忆只有一个开关：`config.memory.enabled`（删除 `persona.memory`，`engine.rs` 的读点同步） | 本次 | 单测：关闭全局开关后不再提取偏好；无重复开关 |
| REQ-S03 | 空闲问候接线：`config.greeting.{enabled,idleMinutes,cooldownMinutes,maxChars}` 生效——空闲达到 `idleMinutes`、距上次问候超过 `cooldownMinutes`、宠物可见且无进行中的对话时触发一次问候；有 DeepSeek 配置走模型，否则用 `persona.greeting` / 时间回落；问候显示为气泡并记录记忆事件；`maxChars` 限制长度 | 本次 | runtime 单测（注入短间隔）覆盖触发、冷却抑制、开关关闭、无凭据回退；`/health` 状态不变 |
| REQ-S04 | Windows 设置信息架构：6 页（宠物 / 外观与交互 / 人格 / 记忆 / 连接 / 系统），单列、分组标题（BodyStrong）、卡片（标题 + 说明 + 右侧控件）、内容最大宽度 ~1000 px | 本次 | 人工：W 表新增设置页条目；自动：dotnet 构建 + 设置页 smoke（N12/N19）不回归 |
| REQ-S05 | 即时生效：删除「保存 DeepSeek 设置 / 保存人格 / 保存记忆配置」；控件变更立即下发；就地反馈（顶部状态条 / 卡片状态文字） | 本次 | 人工：改一项立即生效；自动：SettingsFlowTests 改为逐项命令断言 |
| REQ-S06 | 危险操作确认：删除宠物、清空记忆、覆盖导入用 `ContentDialog` 说明后果 | 本次 | 人工；自动：命令层不变（确认在前端） |
| REQ-S07 | 关于区（系统页底部 expander）：版本、仓库链接、数据目录与日志目录（可点击打开） | 本次 | 人工 |
| REQ-S08 | macOS 同构：`SettingsView.swift` 采用同一 6 分区、卡片化 `Form`/`Section`、即时生效、确认对话框、关于区与问候设置 | 本次（Mac 验证） | Mac 上 `bash scripts/verify-macos-all.sh` 全绿 + 人工；本机只能做代码审查 |
| REQ-S09 | 测试与门禁：Rust workspace、dotnet 测试、`verify-windows.ps1 -Full`、macOS 门禁 | 本次 | 退出码 0，证据入执行记录 |
| REQ-S10 | 文档：`WINDOWS_VERIFICATION.md` / `MACOS_VERIFICATION.md` 设置页条目、`FEATURE_PARITY.md`、执行记录、`AGENTS.md` | 本次 | 文档自查 + 链接有效 |
| REQ-S11 | 宠物页：合并「导入文件夹 / 导入 zip」为单一「导入…」（文件选择器同时接受目录与 `.zip`）；「扫描 Codex 宠物」移入「从 Codex 导入」分区；空列表给空状态文案 | v1.1 | 人工：两种导入都可用、冲突面板不回归 |
| REQ-S12 | 缩放改滑块：0.5–2.0，**吸附原 7 档**（0.5/0.75/1/1.25/1.5/1.75/2.0），拖动实时预览、松手落盘；**托盘菜单同步改**为同一套语义（档位或增减按钮），两边不得冲突 | v1.1 | 自动：dotnet 单测 + smoke N4（窗口尺寸随缩放）；人工：拖动预览与托盘一致性 |
| REQ-S13 | 人格页精简：系统提示词收进「高级」；固定问候并入「连接与问候」的空闲问候卡（作为回退文案）；「回答长度」并入语气预设；删除「简介」与「默认语言」（并删除 `persona.traits.language` / `description` 字段，提示词改为"用与用户相同的语言回答"）；emoji 默认改为开（只影响新建人格，不改老数据） | v1.1 | Rust 单测：schema 兼容 + 提示词不再含「默认语言」；人工：预设 + 自定义语气可用 |
| REQ-S14 | 语气预设：`traits.tone` 提供 4–6 个预设（如 温和/活泼/沉稳/毒舌但温柔），选中即填入文本框且允许继续编辑；**不新增字段** | v1.1 | 人工：预设切换 + 手改都生效 |
| REQ-S15 | 记忆页（第一批）：事实**可编辑**、分级清空（只清事实 / 只清事件 / 全清）、记忆导出 / 导入 | v1.1 | Rust 单测：分级清空与导入导出往返；人工：三条路径各一次 |
| REQ-S16 | 模型供应商：`deepseek` / `custom` 两种；DeepSeek 预填 Base URL；填入 API Key 后可**显式拉取模型列表**（异步、有失败与空状态、允许手填模型名）；自定义 = OpenAI 兼容端点；`thinking` 字段按供应商门控；凭据按供应商隔离（`api-key:<provider>`） | v1.1 | 新增 ABI 命令（`ListModels`，追加值）；Rust 单测：请求体按供应商门控；人工：DeepSeek 拉到列表、自定义手填可用 |
| REQ-S17 | 设置侧边栏：每个分区加图标；窗口变窄时 `NavigationView` 自动收成只显示图标（`PaneDisplayMode=Auto` + Threshold），macOS 侧对应 `NavigationSplitView` 的紧凑行为 | v1.1 | 自动：dotnet build；人工：拉伸窗口观察收纳/展开 |

## 逐文件变更

| 路径 | 变更 | 具体改法 | REQ | 前置条件 |
|---|---|---|---|---|
| `crates/petsona-core/src/persona.rs` | 修改 | 删除 `SamplingConfig` / `ModelRef` / `PersonaMemoryConfig` / `PersonaTtsConfig` / `ProactiveConfig` 与对应字段、校验与单测；保留 `traits` / `greeting` / `systemPrompt` | S01 | — |
| `crates/petsona-core/src/config.rs` | 修改 | 删除 `MemoryConfig.retentionDays` | S01 | — |
| `crates/petsona-runtime/src/commands.rs` | 修改 | `PersonaPatch` 删除对应 13 个字段 | S01 | persona.rs |
| `crates/petsona-runtime/src/engine.rs` | 修改 | `apply_persona_patch` 删除对应分支；`SendConversation` 的记忆门槛改用 `config.memory.enabled`；新增问候调度（空闲/冷却/回退）与 `Greeting` 命令处理 | S01/S02/S03 | commands.rs |
| `crates/petsona-runtime/src/session.rs` | 修改 | 复用 `greeting_rx` / `greeting_inflight` / `last_greeting_at`；如字段不足则补 `last_user_action` 已有则可直接用 | S03 | — |
| `crates/petsona-core/src/deepseek.rs` | 修改 | 若缺问候专用入口，新增 `generate_greeting(..., max_chars)`（沿用 memory context） | S03 | — |
| `apps/windows/Petsona/Views/SettingsWindow.xaml` / `.xaml.cs` | 重写 | 6 页卡片布局、手写 `SettingsCardStyle`、即时生效、`ContentDialog` 确认、关于区、问候设置；删除 14 个控件 | S04–S07 | S01/S02 |
| `apps/windows/Petsona.Tests/SettingsFlowTests.cs` | 修改 | 删除已移除字段断言，改为逐项即时命令断言 + 问候配置断言 | S05/S09 | S01 |
| `apps/macos/Petsona/Sources/SettingsView.swift` | 重写 | 同 6 分区卡片化、即时生效、确认、关于、问候 | S08 | S01/S02 |
| `apps/macos/PetsonaTests/EngineClientTests.swift` | 修改 | 删除 `retentionDays` 断言，补问候配置与人格字段收束断言 | S08/S09 | — |
| `scripts/windows-smoke.ps1` | 修改 | 若设置页结构变化影响 N12/N19 就修；新增问候调度可见性检查（如可行） | S09 | — |
| `docs/WINDOWS_VERIFICATION.md`、`docs/MACOS_VERIFICATION.md`、`docs/FEATURE_PARITY.md`、`docs/execution/settings-consolidation.md`、`AGENTS.md` | 修改 | 新增设置页人工条目、状态与长期规则 | S10 | 实施完成 |

## 必须保持的约束

- **不加第三方依赖**（含 Community Toolkit）；卡片用 `Border` + `Grid` 手写样式。
- **ABI 不变**：人格 / 配置经 JSON 文本跨 FFI，删除字段只改 JSON 形状；`contracts/petsona.h` 与 ABI 版本不动。
- **数据兼容**：`persona.json` / `config.json` 旧文件必须仍可加载（serde 忽略未知字段）；保存后不再写回已删字段。
- 线程/所有权不变：问候调度在 runtime worker 线程，DeepSeek 调用沿用「spawn 线程 + 回传命令」模式，不在 UI 线程阻塞。
- 测试隔离：所有新测试使用独立 `PETSONA_HOME` 与禁用状态服务；问候测试用短间隔注入，不依赖真实 45 分钟。
- 旧 egui / 旧 shell 线冻结：不修改 `crates/petsona-app`、`petsona-shell-*`；它们仍编译通过即可（若引用已删字段，只做最小适配并记录）。

## 验收与命令

| 测试ID | REQ | 目标/架构 | 前置 | 命令或步骤 | 预期 |
|---|---|---|---|---|---|
| T-S01 | S01/S02 | 本机 WSL + Windows cargo | — | `cargo test --workspace` | 退出 0；新增 schema 兼容单测通过 |
| T-S02 | S03 | 同上 | — | `cargo test -p petsona-runtime` | 问候调度单测通过（触发/冷却/关闭/回退） |
| T-S03 | S01–S07 | Windows x64 | VS/WindowsAppSDK | `powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1 -Full` | 退出 0；dotnet 测试 + 25 项 smoke |
| T-S04 | S04–S07 | Windows 桌面 | 验收包 | 人工：打开设置逐页检查卡片、即时生效、确认对话框、关于区 | W 表新增条目全部通过 |
| T-S05 | S08/S09 | macOS arm64 | Mac | `bash scripts/verify-macos-all.sh` | 退出 0（本机无法执行 → 记录为待 Mac 验证） |
| T-S06 | S10 | 文档 | — | 人工检查链接与状态 | 无死链、状态与实际一致 |

## 顺序、暂停与完成

- 顺序：S01/S02（数据模型）→ S03（问候）→ S04–S07（Windows UI）→ S08（macOS UI）→ S09（门禁）→ S10（文档）。
- 可由执行者判断：卡片样式细节、文案、分组内顺序、状态提示形式。
- 必须暂停：若删除字段导致 Codex 人格包导入必需字段缺失；若问候调度需要改变协议 / ABI；若 macOS 改动无法在无 Mac 环境下保证编译。
- 本次完成条件：S01–S04/S06/S07/S10 有实现与自动/人工证据；S05 有即时生效证据；S08 在 Mac 上回归（否则明确「未完成/待 Mac 验证」）；S09 退出码 0。
- 全项目完成条件：Windows 与 macOS 均实机通过设置页人工条目，且旧 schema 兼容证据留存。

## 修订记录

| 版本 | 用户确认依据 | 改变 | 原因 |
|---|---|---|---|
| 1.0 | 2026-09-21：用户逐条确认 5 个决策（手写零依赖 / 接受即时生效 / 死设置完全移除 / 补空闲问候 / 两端同批） | 初版 | 设置项收束与 UI 优化立项 |
| 1.1 | 2026-09-21：第二批用户决定 —— ① 缩放**先按吸附档位**做且**托盘菜单一起改**；② 同意人格页精简（提示词进高级 / 固定问候移到问候卡 / 回答长度并入预设）；③ 同意删除「默认语言」并改为提示词跟随用户语言；④ 同意自定义＝OpenAI 兼容 + `thinking` 门控 + 允许手填模型；⑤ 记忆页**先做**「可编辑 + 分级清空 + 导出/导入」；⑥ 新增：侧边栏图标 + 响应式收纳为纯图标 | 新增 REQ-S11～S17 | 设置页第二批优化 |

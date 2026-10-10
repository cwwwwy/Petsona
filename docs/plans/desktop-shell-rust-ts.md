# desktop-shell-rust-ts：执行契约

## 身份与授权

- 任务ID / 版本 / 日期：`desktop-shell-rust-ts` / v1.0 / 2026-10-08。
- 用户目标及已确认决定（对话依据）：
  - 2026-10-08 用户确认总体方向：**Rust 为唯一系统语言（core / runtime / 平台层 / 浮层窗口），TypeScript 为全部内容界面（设置、聊天、历史、人格、宠物库），WebView 不参与宠物浮层渲染**。
  - 2026-10-08 用户授权：**与最新目标无关的旧代码和文件可以完全摈弃**；`apps/windows`（C#/WinUI）、`apps/macos`（Swift/AppKit）
    与 `crates/petsona-ffi` 不再作为交付物，可从工作树删除，由 git 历史作为唯一归档。
  - 2026-10-08 用户回复“开始”，授权执行 P0 清场立新，随后进入 M0 技术验证。
- 本次范围：P0（提取 → 删除 → 重写根文档与 CI → 门禁）与 M0（技术验证，决策门）。
- 非目标：M0 期间不实现设置/聊天 UI，不改宠物格式、协议、配置与数据目录。
- 后续范围：M1–M6（骨架、浮层 parity、设置、聊天线、macOS、发布）。
- HEAD / 分支：`6bca241`（`main`，工作区干净）——**旧世界的完整快照**，所有删除项均可用只读 `git show` 取回。
- 对应执行记录：`../execution/desktop-shell-rust-ts.md`。

## 目标与需求

### P0：清场立新（本次）

| REQ | 可验证要求 | 范围 | 完成标准 |
|---|---|---|---|
| REQ-D01 | 新文档落盘：本计划、`docs/execution/desktop-shell-rust-ts.md`、新验收清单 `docs/DESKTOP_VERIFICATION.md` | 本次 | 文件存在且内容覆盖本计划提取的旧行为规格 |
| REQ-D02 | 提取旧产品决策：设置信息架构、人格/记忆/问候/模型决策、W 矩阵行为项、协议语义、快照字段语义，全部进入新计划或新验收清单 | 本次 | 见本文「继承的产品决策」；`DESKTOP_VERIFICATION.md` 含协议附录 |
| REQ-D03 | 从工作树删除旧实现（见「逐文件变更」删除清单） | 本次 | 删除项全部消失；其余保留项未被触碰 |
| REQ-D04 | workspace 收为 `petsona-core` + `petsona-runtime`；`Cargo.lock` 同步；根 `Cargo.toml` 的 FFI 注释清理 | 本次 | `cargo metadata` 与 workspace 门禁通过 |
| REQ-D05 | 根文档重写：`AGENTS.md`、`README.md`、`CHANGELOG.md`、`docs/PLATFORM_ARCHITECTURE.md` | 本次 | 与新架构一致，不再引用已删除文件 |
| REQ-D06 | CI 重写：仅保留 Rust 门禁（fmt / clippy / test），删除两端旧 workflow | 本次 | `ci.yml` 只跑 Workspace 门禁；release workflow 待 M6 重建 |

### M0：技术验证（决策门，本次随后启动）

| REQ | 可验证要求 | 范围 | 完成标准 |
|---|---|---|---|
| REQ-S01 | `apps/desktop` 骨架：Tauri 2 壳 + Vite/React/TS，可启动一个空设置窗（Windows 实机） | M0 | 窗口可开合，无控制台报错 |
| REQ-S02 | Rust 原生宠物窗：`WS_EX_LAYERED` + `UpdateLayeredWindow` 渲染图集帧；拖动跟手 | M0 | 实机目视 + 量化记录 |
| REQ-S03 | 逐像素命中/穿透：透明像素落到桌面、宠物像素可点击；光标为普通箭头 | M0 | 对照现版 N9/N15/N23/N24 方法 |
| REQ-S04 | 焦点语义：宠物窗不抢焦点；Composer/设置窗可前台聚焦并直接输入 | M0 | 对照 W13 |
| REQ-S05 | 托盘：任务栏可见与收纳面板均能右键出菜单 | M0 | 对照 W2/N17 |
| REQ-S06 | 启动耗时：热/冷启动记录，对照基线（热 ~0.55s / 冷 ~3.2s，E-W15d） | M0 | 数字入执行记录，不劣化即过；劣化需归因 |
| REQ-S07 | macOS 极小可行性：objc2 创建透明 NSPanel 并绘制一帧 | M0 | 结论（可行/受阻）入执行记录；玻璃材质留 M5 |
| REQ-S08 | 决策结论：六项对照全部通过 → 进入 M1；任一项不过 → 记录根因、给出对策；无法解决时暂停交回用户 | M0 | 执行记录含逐项结论 |

### M1–M6：后续里程碑（范围概述，启动时细化 REQ）

| 里程碑 | 内容 |
|---|---|
| M1 骨架 | 单实例、数据目录/日志、协议（17872 + 测试端口）、完整托盘菜单、空库首启开设置 |
| M2 浮层 | 宠物动画/注视/拖动/位置持久化/工作区夹取、气泡（淡入/进度/悬停暂停）、编辑条、Composer |
| M3 设置 | TS 六页：宠物 / 外观与交互 / 人格 / 记忆 / 连接与问候 / 系统 |
| M4 聊天线 | 流式聊天、历史查看/清除、人格来源、记忆审阅 |
| M5 macOS | objc2 浮层、玻璃材质、Dock 显隐、LaunchAgent、Keychain |
| M6 发布 | 打包（Windows 自包含/WebView2 策略、macOS 签名公证）、release workflow、版本与文档 |

## 逐文件变更

### 删除清单（P0）

| 路径 | 说明 |
|---|---|
| `apps/windows/` | C#/WinUI 3 前端、Core、Tests、sln（约 6.9k 行 C# + 717 行 XAML） |
| `apps/macos/` | SwiftUI/AppKit 前端、Xcode 工程、Tests（约 4.8k 行 Swift） |
| `crates/petsona-ffi/` | C ABI，仅为旧前端存在 |
| `contracts/` | `petsona.h`、`ABI.md`（有效语义已提取到新文档） |
| `scripts/`（全部） | 旧平台验证、smoke、打包、签名、公证、LaunchAgent 安装、诊断脚本 |
| `.github/workflows/release-windows.yml`、`release-macos.yml` | 旧发布流程，M6 重建 |
| `docs/plans/` 六条旧线 | `macos-companion-evolution`、`native-ui-rewrite`、`persona-memory-reshape`、`settings-consolidation`、`windows-native-rewrite`、`windows-overlay-interaction` |
| `docs/execution/` 全部旧记录 | 含 `macos-companion-evolution-tests.json`、`windows-release-0.1.0-rc.1.md` |
| `docs/FEATURE_PARITY.md`、`docs/MACOS_VERIFICATION.md`、`docs/WINDOWS_VERIFICATION.md` | 行为规格已提取到 `DESKTOP_VERIFICATION.md` |
| `packaging/macos/com.petsona.desktop.plist`、`packaging/windows/generate-icon.ps1` | 旧打包工具；图标资产保留 |

### 保留清单

- `crates/petsona-core/`、`crates/petsona-runtime/`、`crates/petsona-core/testdata/` 测试夹具。
- 品牌资产：`packaging/macos/Petsona.icns`、`packaging/windows/Petsona.ico`。
- `docs/plans/TEMPLATE.md`、`docs/execution/TEMPLATE.md`、`LICENSE`、`rust-toolchain.toml`。

### 重写清单（不删除）

- `AGENTS.md`：新架构、新现状、新命令；保留仍然有效的平台坑与教训（详见下文）。
- `README.md`：项目定位、当前状态（重建中）、目标结构与开发命令。
- `CHANGELOG.md`：旧 rc.1 条目作废，重建新历史。
- `docs/PLATFORM_ARCHITECTURE.md`：Rust 层 + TS 层职责边界、线程模型、Rust↔TS 接口原则。
- `.github/workflows/ci.yml`：仅 Rust Workspace 门禁。
- `Cargo.toml`（移除 ffi 成员与注释）、`.gitignore`（清理旧平台条目）。

### 新增清单

- `docs/plans/desktop-shell-rust-ts.md`（本文）、`docs/execution/desktop-shell-rust-ts.md`、`docs/DESKTOP_VERIFICATION.md`。

## 必须保持的约束

- **数据兼容**：数据目录（`%APPDATA%\Petsona`、`~/Library/Application Support/Petsona`）、`config.json` 格式、宠物库、
  `petsona.lock` 单实例、协议语义不因重建而改变；用户数据零迁移。
- **协议语义延续**：`POST /state`、`GET /health`、`GET /pets`；source 归属与覆盖规则；TTL 与 `action:"clear"`（见验收清单附录）。
- **浮层与内容边界**：宠物 / 气泡 / 编辑条 / Composer 必须原生渲染（Rust）；设置 / 聊天 / 历史 / 人格 / 宠物库为 WebView 内容页面。
- **测试隔离**：`PETSONA_HOME`、专用端口、自启值名、假凭据；不得让测试启动默认用户实例。
- **依赖策略**：本任务批准引入 Tauri 2 + Vite/React/TS 生态；版本锁定并提交 lockfile；`petsona-core/runtime` 仍优先标准库与现有依赖。
- **Git 只读**：AI 不执行 add/commit/push/tag；每批改动给出完整未提交改动摘要与提交命令。
- **旧约束的变更**：原“一个仓库一个 workspace、两端原生前端”的架构约束被本计划取代；原 macOS 最低版本 26、
  webp 优先、无内置宠物、物理像素几何等业务约束继续有效。

## 继承的产品决策（提取自旧文档，供 M2–M5 实现与验收）

### 设置信息架构（settings-consolidation v1.2）

- 六页：宠物 / 外观与交互 / 人格 / 记忆 / 连接与问候 / 系统；卡片式分组（标题 + 说明 + 右侧控件），内容最大宽度约 1000px。
- **即时生效**：无保存按钮；危险操作（删除宠物 / 清空记忆 / 覆盖导入）必须有确认。
- 系统页：版本、仓库链接、数据目录与日志目录（可打开）。
- 宠物页：合并“导入文件夹/导入 zip”为单一“导入”；“扫描 Codex 宠物”位于“从 Codex 导入”分区；空列表有空状态文案；
  本地列表单击仅选择、**双击才切换**；Codex 候选双击导入。
- 缩放：0.5–2.0 滑块，吸附 0.5/0.75/1/1.25/1.5/1.75/2.0，拖动实时预览、松手落盘；托盘菜单同语义。
- 侧边栏：每页图标；窗口变窄时收纳为仅图标。
- 模型供应商：`deepseek`（预填 Base URL，填 Key 后可拉取模型列表，有失败/空状态，可手填）与 `custom`（OpenAI 兼容端点）；
  凭据按供应商隔离（`api-key:<provider>`）；信息顺序：服务商 → URL → API Key → 模型 → 高级。
- 空闲问候归“外观与交互”：启用开关、固定文案、空闲分钟、冷却、最大字数；无凭据时用固定文案/时间回落。
- 人格页 = “说话方式”，只作用于当前宠物：语气预设（4–6 个，可继续手改）+ emoji（默认开，只影响新建）+（高级）系统提示词；
  操作：复制到… / 导入 / 导出 / 重置为内置。
- 记忆（按宠物保存）：事实可编辑；分级清空（事实 / 事件 / 全部）；导入导出；事实显示来源（手动 / 对话 / 导入 / 压缩）；
  隐私说明（仅本机、仅配置模型服务才出网）；事件保留天数与长期事实压缩；删除宠物不静默删除其人格文件与记忆。
- macOS（M5 参考）：设置窗打开时显示 Dock 图标、关闭后恢复；六页原生分组表单语义延续到 TS 实现的目标是**行为等价**而非控件等价。

### 浮层与交互（windows-overlay-interaction v1.0）

- 宠物边界：拖动 / 恢复位置 / 尺寸变化统一夹取到当前显示器工作区（任务栏为边界）。
- 气泡：150ms 淡入；底部剩余时间进度条；悬停暂停、移开从剩余时间续跑；新气泡重置；进度与 TTL 由 runtime 计算。
- 编辑条：宠物下方优先；贴底时移至左右空间较大一侧并旋转；悬停约 220ms 展开并聚焦；点击立即打开。
- Composer：无标题栏 / 无边框 / 无快捷键提示；只显示输入区与圆形发送按钮；拖动时跟随宠物；Enter 发送 / Shift+Enter 换行 /
  Esc 关闭保留草稿；中文 IME 组合不误发。
- 托盘：图标（含收纳面板与固定到任务栏两种状态）左/右键弹出菜单；Esc / 点外关闭；菜单项含设置、历史、宠物、缩放、显隐、退出。

### 协议与状态语义（DESKTOP_VERIFICATION 附录为准）

- 状态优先级：`failed` 90 > `waiting` 80 > `running` 70 > `review` 60 > `waving`/`jumping` 40 > look 行 20 >
  `running-left/right` 10 > `idle` 0；**同一 source 必然覆盖自己**；不同 source 需严格更高优先级。
- `ttlMs:0` 表示不过期；`action:"clear"`（大小写不敏感、去除首尾空白）解除该 source 自己的覆盖；同 body 同时带 `state` 与 `clear` 时 clear 优先。
- 用户交互（点击 `waving`、拖动 `running-left/right`）不打断 agent 状态；Composer 聊天只发对话、不推状态。
- 快照字段语义（BubbleTiming、toast、pet 状态、geometry 等）以 `crates/petsona-runtime` 现有投影为准，M1 迁移为 Tauri 事件。

### 平台坑与教训（保留至 AGENTS.md）

持有窗口线程必须抽消息、Win11 1px 边框修复、命中像素随姿态变化的并集掩码、物理像素位置记忆、HKCU Run 受限、窗口光标接管、
`HTTRANSPARENT` 同线程限制、协议 socket 非阻塞继承、WSL PATHEXT、PowerShell 5.1 BOM、macOS 代码必须在 Mac 上验证。

## 验收与命令

| 测试ID | REQ | 目标程序/架构 | 前置环境 | 命令或人工步骤 | 预期结果/证据 |
|---|---|---|---|---|---|
| T-D01 | REQ-D01/D02 | 仓库 | WSL | 文件清单核对（`ls docs`、`git status --short`） | 新文档存在；旧文档消失 |
| T-D02 | REQ-D03/D04 | Rust workspace | WSL | `cargo fmt --all -- --check`；`cargo clippy --workspace --all-targets -- -D warnings`；`cargo test --workspace` | 全部 exit 0，测试数记录入执行记录 |
| T-D03 | REQ-D05/D06 | 文档与 CI | WSL | 文档链接自查；`ci.yml` 语法检查（YAML 解析） | 无失效引用；YAML 有效 |
| T-S01..S08 | REQ-S01..S08 | `apps/desktop`（Windows x64 实机） | Windows 桌面 + WebView2 | 见 `docs/DESKTOP_VERIFICATION.md` M0 章节 | 六项对照结论 + macOS 可行性结论 |

## 顺序、暂停与完成

- 工作依赖顺序：P0 提取（D01/D02）→ 删除（D03）→ workspace/CI/根文档（D04–D06）→ 门禁 → M0 骨架 → 实机验证。
- 可由执行者判断的局部实现选择：新进程内线程模型、Tauri 托盘 vs 原生托盘（先试 Tauri）、目录命名细节。
- 必须暂停并交回用户的冲突：M0 六项中任一无法达到基线且无明确对策；需要改变产品行为或数据格式时。
- 本次完成条件（P0）：REQ-D01–D06 全部满足，T-D01–T-D03 通过。
- 全项目完成条件（M0）：REQ-S01–S08 有明确结论；六项通过后进入 M1（M1+ 另行细化 REQ 并延续本计划版本号）。

## 修订记录

| 版本 | 用户确认依据 | 改变的REQ/范围/验收 | 原因 |
|---|---|---|---|
| v1.0 | 2026-10-08 对话“开始” | 初始版本：P0 清场（含完整删除清单）+ M0 决策门 | 用户确认 Rust+TS 方向并授权摈弃旧线 |

## 2026-10-10 接续决定

用户确认先完成设置页深度修复，再推进M5；此前M5暂缓更新。设置专项契约见 `settings-ui-deep-review.md` v1.0，Mac细化契约见 `desktop-m5-macos.md` v1.0；两者各自保持必需验收标准，实施/证据分别记录。

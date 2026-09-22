# persona-memory-reshape：执行契约

> 由 2026-09-22 对话确认：用户认可「方案 A（一宠一人格）」+ 人格/记忆重塑方向。

## 身份与授权

- 任务ID / 版本 / 日期：`persona-memory-reshape` / `1.0` / 2026-09-22。
- 用户目标：**人格与宠物直接绑定**（切换宠物即切换说话方式），并把人格/记忆功能重塑为更贴近桌宠的模型。
- 已确认决定（2026-09-22 对话）：
  1. 采用 **方案 A：一宠一人格**——每只宠物有自己的「说话方式」，切换宠物自动切换；
  2. 迁移策略：首次启动把当前 `activePersona` 绑定到当前宠物，其余宠物默认使用内置「小助手」说话方式；
  3. 方向认可：记忆跟随宠物；记忆透明（来源/时间线）；记忆可控（暂停记录、重置）；记忆自动（事件过期、长期摘要）；说话方式预设可扩展并允许保存为用户预设；隐私说明。
- 非目标：不改 Codex 宠物包格式（说话方式仍存 Petsona 数据目录，不写回宠物包）；不做云同步。
- 执行记录：`../execution/persona-memory-reshape.md`。

## 目标与需求

| REQ | 可验证要求 | 范围 | 完成标准 |
|---|---|---|---|
| REQ-P01 | 数据模型：`config.json` 新增 `personaByPet`（pet id → persona id）；启动时若为空且 `activePersona` 非默认，则把它绑定到当前宠物（一次性迁移） | 本次 | Rust 单测：新配置默认为空、迁移只发生一次、旧配置可加载 |
| REQ-P02 | 运行时：`SelectPet` 后解析该宠物绑定的人格（无绑定 → 内置小助手）；`SelectPersona` 会把选择写回该宠物的绑定 | 本次 | runtime 单测：切宠物换人格、绑定写回、重启后保持 |
| REQ-P03 | UI（两端）：人格页改为「说话方式」，只作用于**当前宠物**——语气预设 + 自定义、emoji、（高级）系统提示词；移除人格选择器与「新建/删除」；保留「复制到…/导入/导出/重置为内置」作为当前宠物说话方式的操作 | 本次 | 人工：切宠物看到不同说话方式；导入导出可用 |
| REQ-P04 | 记忆跟随宠物：删除宠物时清理该宠物的记忆（或明确保留策略）；设置页说明"记忆按宠物保存" | 本次 | runtime 单测：删除宠物后的记忆状态明确且可预期 |
| REQ-P05 | 记忆自动：复活"事件保留天数"（按天清理旧事件）与长期事实压缩（超过阈值时把最旧事实合并为一条画像）；设置页给出开关 | 后续 | 单测 + 人工 |
| REQ-P06 | 透明度与隐私：事实显示来源（手动 / 对话自动）；设置页写明"数据只在本机、只有配置模型服务才出网"；宠物卡片提供「清空这只宠物的记忆」 | 后续 | 人工 |

## 逐文件变更

| 路径 | 变更 | 具体改法 | REQ |
|---|---|---|---|
| `crates/petsona-core/src/config.rs` | 修改 | `AppConfig` 新增 `persona_by_pet: BTreeMap<String, String>`（serde 默认空） | P01 |
| `crates/petsona-runtime/src/session.rs` | 修改 | 新增 `persona_for_pet` 解析与迁移辅助 | P01/P02 |
| `crates/petsona-runtime/src/engine.rs` | 修改 | 加载时迁移；`SelectPet` 后切换人格；`SelectPersona` 写回绑定；`DeletePet` 时处理记忆 | P01/P02/P04 |
| `crates/petsona-runtime/src/commands.rs` | 修改 | 如需新增 `ResetPersona`（当前宠物说话方式重置为内置） | P03 |
| `apps/windows/Petsona/Views/SettingsWindow.xaml(.cs)` | 修改 | 人格页 →「说话方式」；移除选择器 / 新建 / 删除；复制到… / 导入 / 导出 / 重置保留 | P03 |
| `apps/macos/Petsona/Sources/SettingsView.swift` | 修改 | 同上（`EngineClient` 如需新命令同步） | P03 |
| `docs/WINDOWS_VERIFICATION.md` / `docs/MACOS_VERIFICATION.md` / `FEATURE_PARITY.md` | 修改 | 新增人工项与状态 | P03 |

## 必须保持的约束

- **不改宠物包格式**：说话方式只写 Petsona 数据目录；导出的人格文件仍是既有 `persona.json` 形状（可移植）。
- 兼容：旧 `config.json`（无 `personaByPet`）必须可用；`activePersona` 字段保留（迁移后仍写当前值）。
- 记忆按 persona 分桶的既有实现不变（绑定后语义自然变成"按宠物"）；不得在未确认的情况下删除用户记忆。
- 两端同批，macOS 仍以 Mac 门禁为准。

## 验收与命令

| 测试ID | REQ | 目标 | 命令/步骤 | 预期 |
|---|---|---|---|---|
| T-P01 | P01/P02/P04 | Rust | `cargo test -p petsona-runtime -p petsona-core` | 新增绑定 / 迁移 / 删除宠物测试通过 |
| T-P02 | P03 | Windows | `verify-windows.ps1 -Full` | exit 0；dotnet 与 smoke 不回归 |
| T-P03 | P03 | Windows 桌面 | 人工 W24：切宠物 → 说话方式与记忆随之切换；导入导出仍可用 | 通过 |
| T-P04 | P03 | macOS | `bash scripts/verify-macos-all.sh` | 退出 0（本机无法执行 → 标注待 Mac 验证） |

## 顺序、暂停与完成

- 顺序：P01 → P02 → P03（两端）→ P04 → 文档；P05/P06 另起一轮。
- 可由执行者判断：说话方式页的具体排版、复制/导入/导出的按钮位置。
- 必须暂停：若绑定需要改变宠物包格式或 ABI 布局；若删除宠物的记忆策略无法在"不误删"的前提下明确。
- 本次完成条件：P01–P04 有实现与自动证据；P03 的 macOS 部分标注待 Mac 验证。
- 全项目完成条件：P05/P06 完成后关闭本任务。

## 修订记录

| 版本 | 用户确认依据 | 改变 | 原因 |
|---|---|---|---|
| 1.0 | 2026-09-22：用户认可方案 A 与人格/记忆重塑方向 | 初版 | 人格与宠物绑定 + 记忆重塑立项 |

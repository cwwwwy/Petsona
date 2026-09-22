# persona-memory-reshape：执行与审查记录

## 接手

- 计划：[docs/plans/persona-memory-reshape.md](../plans/persona-memory-reshape.md) v1.0（2026-09-22 用户认可方案 A + 重塑方向）。
- 授权：2026-09-22「认可 A + 上述方向，同步开始执行」。
- 前置：`settings-consolidation` 的 S01–S17 已实施（设置项收束、卡片化、即时生效、语音/记忆/模型服务等）。

## 要求与文件

| REQ | 实现状态 | 自动验证 | 人工验收 |
|---|---|---|---|
| REQ-P01 数据模型 + 迁移 | 已实现：`AppConfig.personaByPet`（`BTreeMap<petId, personaId>`，serde 默认空）；启动时若映射为空且存在 activePet，则把当前 persona 绑定到该宠物并**立即落盘**（迁移只发生一次） | `cargo test -p petsona-runtime` **12/12**（新增 `first_load_binds_the_active_persona_to_the_active_pet`） | 不适用 |
| REQ-P02 运行时按宠物解析说话方式 | 已实现：加载时**宠物绑定优先于 `activePersona`**（重启回到该宠物的风格）；`SelectPet` 有绑定则跟随、无绑定则**复制当前风格成这只宠物自己的 persona**（id = pet id；已存在同名文件则复用，不覆盖）；`SelectPersona` 写回绑定 | `cargo test -p petsona-runtime`（新增 `speaking_style_is_bound_to_the_pet_and_restored_after_restart`：绑定写回 + 重启后绑定胜出） | 待人工（W24） |
| REQ-P03 UI：人格页 → 说话方式 | Windows/macOS 均改为「说话方式」，只作用于当前宠物：语气预设 + 自定义、emoji、（高级）系统提示词；**移除**人格选择器与新建/复制/删除/名称；保留导入… / 导出… / **重置为内置**（新增 ABI 命令 42 `ResetPersona`：恢复内置风格，保留 persona id 与绑定，不动记忆） | Windows：dotnet 37/37、smoke 25/25；macOS：完整门禁 XCTest 14/14（E-28d） | 待人工（W24/W25；Mac 宠物绑定说话方式/导入导出仍待实机） |
| REQ-P04 删除宠物的记忆策略 | 已实现（策略 = **保留**）：删除宠物只移除 `personaByPet` 绑定；persona 文件与记忆**不静默删除**（清空入口留给 P06 的显式操作） | 代码审查 + 计划记录 | 待人工（W26 说明文案） |
| REQ-P05 记忆自动（过期 / 压缩） | 已实现：`MemoryConfig.eventRetentionDays`（0 = 永久，启动与改配置时按天清理旧事件）与 `factCompress`（默认开，超出 `factLimit` 时把较旧的事实合并成一条「画像」事实，**替代原来的静默截断**）；来源通过 `Fact.source` 记录 | core 71/71、runtime 14/14；macOS 原生 Release/XCTest 见 E-28d | 待人工（W27；Mac 设置页检查待做） |
| REQ-P06 透明度与隐私（来源标注 / 清空入口 / 隐私说明） | 已实现：事实列表标出来源（手动 / 对话 / 导入 / 压缩）；「清空这只宠物的记忆」文案明确作用域；记忆卡片补隐私说明（本机保存、仅配置模型服务时才出网） | Windows `-Full` / smoke 25/25；macOS 原生编译与 XCTest 见 E-28d | 待人工（W28；Mac 设置页检查待做） |

## 命令证据

| 证据ID | 内容 | 结果 |
|---|---|---|
| E-P01 | `cargo test -p petsona-runtime` | **12/12**：新增绑定迁移、绑定持久化/重启恢复；既有协议、问候、记忆、模型列表测试不回归 |
| E-P02 | `verify-windows.ps1` | exit 0；cargo 全量 + dotnet **37/37** + MSVC FFI SHA256 守卫 |
| E-P03 | `windows-smoke.ps1` | **25/25 PASS**（设置窗结构大改后空库首启与聚焦不回归） |
| E-P04 | 桌面验收包刷新 | `Desktop\Petsona-验收-修复版\`（59 文件，`petsona_ffi.dll` md5 `75345bff2de0a4526f4c559c1186043c`） |

## 第十轮补充：P05/P06（2026-09-22）

| 证据ID | 内容 | 结果 |
|---|---|---|
| E-P05 | `cargo test -p petsona-core` | **70/70**：事件过期只清理超期事件（0 天 = 永久保留）；事实压缩把最旧的事实并入「画像」且**不产生重复画像**（两次压缩后仍只有一条） |
| E-P06 | `verify-windows.ps1` | exit 0（dotnet 37/37 + FFI 守卫） |
| E-P07 | `windows-smoke.ps1` | 首轮 N18 FAIL（合成点击未生效）→ 复核桌面可用（截图正常、光标被用户移动）后**重跑 25/25 PASS**，判定为桌面输入干扰而非回归；保留失败记录 |
| E-P08 | 桌面验收包刷新 | md5 见交付说明（每轮刷新） |

- 记忆策略细节：`eventRetentionDays` 默认 0（不改变老用户行为）；`factCompress` 默认开——原来超出上限是**静默截断**（丢数据），现在改为合并成「画像」事实，属于行为改进且不删数据。

## 偏差与说明（执行者主动记录）

- 计划 REQ-P03 原写"保留「复制到…」"：在方案 A（一宠一人格）下已无"目标人格列表"，该操作失去语义，故**未实现**；若需要"把 A 宠物的说话方式复制给 B 宠物"，应作为新需求提出（当前可通过导出 A、导入到 B 实现）。
- macOS 侧：说话方式页已改，旧辅助（`showingNewPersona` sheet、`duplicateCurrentPersona`、`personaName` 状态）仍属**待清理**。2026-09-22 已完成 macOS Release 编译及完整自动门禁（E-28d）；人工体验尚未验收。
- 迁移会**立即写盘**：这是为了让迁移只发生一次；若用户随后手工删除映射，重启后会重新按 `activePersona` 解析。

## 审查及关闭

| REV/优先级 | REQ | 位置/触发/影响 | 必需修复与测试 | 关闭证据/状态 |
|---|---|---|---|---|
| — | — | 尚无审查项 | — | — |

## 交付与接续

- 实际完成范围：REQ-P01～P06 已实施；Windows 自动门禁通过；macOS 同构设置已编译并通过完整自动门禁（E-28d）。
- 未完成/待验收：W24（切宠物换说话方式 + 记忆随之切换）、W25（重置为内置 / 导入导出）、W26（删除宠物后记忆保留的说明）、W27（记忆过期/压缩）、W28（来源与隐私）；macOS 侧对应设置和说话方式绑定的人工检查仍待做。
- 对用户数据的影响：`config.json` 新增 `personaByPet`（旧文件可加载）；首次启动会写入一条绑定；**不会**删除任何 persona 或记忆数据。
- Git 操作是否发生（默认无）：无。
- 完成判定：**未完成**（功能已有实现且 macOS 自动门禁通过；W24–W28 与 Mac 实机验收未闭合）。

### 代码审查补充（2026-09-22）

- 凭据状态现有可注入的 `SecretStore` 检查函数，Rust 测试覆盖空 store → 未配置、memory store 有凭据 → 已配置；runtime 测试覆盖 no-key 投影与固定问候，不访问系统凭据。
- XCTest Host 的启动隔离和真实用户目录未变化守卫见 `native-ui-rewrite` E-28d。Mac UI 的“未配置”显示已加投影标签 XCTest；人工 Keychain / 设置体验仍待验。
- 上述共享 runtime no-key 回退改动只在 Mac / Rust 环境验证；Windows `-Full` 尚待复跑，不能记为双端通过。

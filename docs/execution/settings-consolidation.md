# settings-consolidation：执行与审查记录

## 接手

- 计划路径/版本：[docs/plans/settings-consolidation.md](../plans/settings-consolidation.md) v1.0（2026-09-21 用户逐条确认 5 个决策后落盘）。
- 用户授权 / 日期：2026-09-21「按你的计划执行」+ 5 项决定（手写零依赖卡片、接受即时生效、14 个死设置完全移除、补空闲问候、两端同批）。
- 基线：`main`；执行前工作区含上一轮的 CR-W2 协议改动与文档归档（未提交），本轮在其上继续。
- 执行环境：WSL + Windows 11（`DESKTOP-UI2ET48`），VS Build Tools 18 提供 MSVC 链接器。

## 要求与文件

| REQ | 实现状态 | 自动验证 | 人工验收 |
|---|---|---|---|
| REQ-S01 删除 14 个无行为字段 | 已实现：`persona.rs` 删除 `avatarPet` / `sampling` / `model` / `memory` / `tts` / `proactive` 及其结构体与校验；`config.rs` 删除 `memory.retentionDays`；`PersonaPatch` 同步收缩 | 通过：`cargo test -p petsona-core` **60/60**（新增 `legacy_persona_files_still_load_without_the_removed_fields`：旧 JSON 可加载、保存后不再写回已删字段） | 不适用 |
| REQ-S02 记忆只保留全局开关 | 已实现：`engine.rs` 的记忆门槛改为 `config.memory.enabled`；`persona.memory` 整个删除 | 通过：core + runtime 测试同轮 | 不适用 |
| REQ-S03 空闲问候 | 已实现：`tick_runtime` 每帧检查 `greeting_due`（开关 / `idleMinutes` / `cooldownMinutes` 纯函数），到期后在独立线程调用 `DeepSeekClient::generate_greeting(..., maxChars)`，结果经 `RuntimeCommand::GreetingResult` 回到 worker：气泡 + 一次性 `waving` + `EventKind::PetGreeting` 记忆事件；失败回退 `fallback_greeting`；点击 / 拖动 / 发送对话都刷新 `last_user_action` | 通过：`cargo test -p petsona-runtime` **8/8**（`greeting_due_respects_the_switch_idle_window_and_cooldown`、`greeting_result_shows_a_bubble_and_records_memory` 含无凭据回退） | 待人工（30 分钟空闲或短间隔配置） |
| REQ-S04 设置信息架构（6 页卡片） | 已实现：`SettingsWindow.xaml` 重写为 宠物 / 外观与交互 / 人格 / 记忆 / 连接与问候 / 系统，全部 `Border` 卡片（手写 `SettingsCardStyle`），分组标题 + 说明文字，`MaxWidth=1000` 单列，窗口底部状态条 | 通过：dotnet build（`TreatWarningsAsErrors`）0 警告 + XAML 编译 | **2026-09-21 用户实机通过**（W15） |
| REQ-S05 即时生效 | 已实现：删除三个「保存」按钮与处理函数，改为构造函数里逐控件挂 `ScheduleApply`（文本框 450 ms 去抖，其余立即），`_suppressEvents` 阻止回填触发；人格保存后刷新列表投影 | 通过：dotnet build + `SettingsFlowTests` 36/36（含 `UpdateGreetingConfig` 往返断言） | **2026-09-21 用户实机通过** |
| REQ-S06 危险操作确认 | 已实现：删除宠物 / 清空记忆沿用 `ConfirmAsync`（`ContentDialog`）；覆盖导入面板保留在「宠物」页并说明后果 | 通过：编译 + 现有测试 | **2026-09-21 用户实机通过** |
| REQ-S07 关于区 | 已实现：系统页「关于」卡片显示版本与项目主页；数据目录 / 日志目录卡片可直接打开（`PETSONA_HOME` 优先） | 通过：编译 | **2026-09-21 用户实机通过** |
| REQ-S08 macOS 同构 | 已实现（待 Mac 编译）：`SettingsView.swift` 删除全部死字段与三个「保存」按钮，改为 `personaSignature` / `deepSeekSignature` / `memorySignature` / `greetingSignature` + 450 ms 去抖的即时生效；分区改名为「连接与问候」「系统」；新增空闲问候四项；系统页新增数据目录与关于区；`EngineClient.updateGreetingConfig` 已就位。macOS 用原生 `Form`/`Section` 分组（等价于 Windows 卡片，不引第三方样式） | 未跑（本机无 macOS 工具链） | 待 Mac：`bash scripts/verify-macos-all.sh` + 人工检查 |
| REQ-S09 门禁 | 通过（Windows）：`verify-windows.ps1 -Full` **exit 0** —— cargo fmt/clippy/**95 项测试** + MSVC FFI DLL SHA256 守卫 + dotnet **36/36** + native smoke **25/25 ×2**（源码 + 解压包，0 SKIP）+ 打包结构检查 | E-S02/E-S06 | Windows 人工条目 W15 待用户确认；macOS 门禁待 Mac |
| REQ-S10 文档 | 部分：本文件 + 计划 + `contracts/ABI.md`；`WINDOWS_VERIFICATION.md` / `MACOS_VERIFICATION.md` / `FEATURE_PARITY.md` / `AGENTS.md` 待补 | 待做 | 待做 |

## 命令证据

| 证据ID | 内容 | 结果 |
|---|---|---|
| E-S01 | `cargo test --workspace`（Windows，`verify-windows.ps1` 内） | exit 0；core 60、app 25、runtime 8、ffi 3、shell-windows 2 |
| E-S02 | `verify-windows.ps1`（非 `-Full`） | exit 0；cargo fmt/clippy（`-D warnings`）/test/release + MSVC `petsona_ffi.dll` + app 输出 SHA256 守卫 + dotnet restore/build/format/test **36/36** |
| E-S03 | 失败记录：cargo clippy | `field_reassign_with_default`（新增测试写法）→ 改为结构体更新语法；保留失败记录 |
| E-S04 | 失败记录：XAML 编译 | `WMC0011: Unknown member 'Resources' on element 'Window'`（WinUI 3 的 `Window` 无 `Resources`）→ 资源字典移到根 `Grid.Resources`；保留失败记录 |
| E-S05 | smoke 阻断诊断（2026-09-21） | `windows-smoke.ps1` 两次在 N18 失败并中止（`CopyFromScreen` 句柄无效）。隔离探针（`.scratch/composer-probe.ps1`，已清理）定位根因：`WindowFromPoint(编辑按钮中心)` 返回 `LockScreenBackstopFrame` —— **桌面处于锁屏状态**，合成鼠标事件落在锁屏上；与本次改动无关。用户解锁后 `-Full` 一次通过（见 E-S06） |
| E-S06 | `verify-windows.ps1 -Full`（解锁后） | exit 0；native smoke **25/25 PASS、0 SKIP**（源码构建 + 解压包各一轮），dotnet 36/36，FFI SHA256 守卫与打包结构全绿；日志 `/tmp/verify-settings-full.log` |
| E-S07 | 桌面验收包刷新 | `scripts\package-windows.ps1` exit 0；`Desktop\Petsona-验收-修复版\Petsona-windows-x64-0.1.0\` 已替换为新构建（59 文件，`petsona_ffi.dll` md5 `5a9ec34922d856fe6593706a4bef6a77`），旧包改名保留 |

## 审查及关闭

| REV/优先级 | REQ | 位置/触发/影响 | 必需修复与测试 | 关闭证据/状态 |
|---|---|---|---|---|
| — | — | 尚无审查项（审查对话未开始） | — | — |

## 交付与接续

- 实际完成范围：REQ-S01～S08 实现；REQ-S09 Windows 侧全部通过（`-Full` exit 0）；REQ-S10 部分（计划 / 执行记录 / ABI / W15 已落盘，`MACOS_VERIFICATION.md` 与 `FEATURE_PARITY.md` 待补）。
- 未完成/失败/SKIP/人工待验：**macOS 编译与门禁**（改动只能在 Mac 上验证）；**W15 人工检查**（新设置页逐页 + 即时生效 + 空闲问候观感）；`MACOS_VERIFICATION.md` / `FEATURE_PARITY.md` 收尾（S10）。
- 对用户数据的影响：`persona.json` / `config.json` 不再写出已删字段，旧文件仍可加载；新增 `UpdateGreetingConfig`（ABI 命令值 36，追加式，ABI 版本仍为 3）。
- 下一个执行者的安全接续点：在 Mac 上跑 `bash scripts/verify-macos-all.sh`，修掉可能的 Swift 编译问题（去抖 / `onChange` 签名是最可能出问题的两处），并完成 macOS 侧设置页人工检查；Windows 侧只剩 W15 人工确认。
- Git 操作是否发生（默认无）：无。
- 完成判定：**未完成**（macOS 未经编译/门禁、S10 未收尾；Windows 自动门禁 + W15 人工均已通过）。

### 第二批进度（v1.1）

- **REQ-S17 已完成**（2026-09-21）：`SettingsWindow.xaml` 侧边栏六个分区各加 `SymbolIcon`（Folder / Font / Contact / Clock / World / Setting），`PaneDisplayMode="Auto"` + `CompactModeThresholdWidth=720` + `ExpandedModeThresholdWidth=1000`，窗口变窄自动收成纯图标栏。证据：`verify-windows.ps1` exit 0（cargo 95 项 + dotnet 36/36 + FFI 守卫），桌面验收包已刷新为含图标的构建（`petsona_ffi.dll` md5 `2cd17c66b3a843186f87b017942349ee`）。
- **REQ-S11～S16 待实施**（计划 v1.1）：宠物页导入合并、缩放滑块 + 托盘同步、人格页精简（含 `traits.language` / `description` 字段删除与提示词改写）、语气预设、记忆页「编辑 + 分级清空 + 导出导入」、模型供应商（DeepSeek / 自定义 + 拉取模型列表 + `thinking` 门控 + 凭据按供应商隔离）——均需两端同批，macOS 侧待 Mac 验证。

## 下一批（用户 2026-09-21 提出的设置页微调，已在计划 v1.1 落盘）

1. 宠物页：合并「导入文件夹 / 导入 zip」为单个「导入…」；「扫描 Codex 宠物」移入 Codex 分区。
2. 外观与交互页：缩放档位改为滑块（需保留托盘菜单一致性 + 吸附档位 + 实时预览）。
3. 人格页：语气改预设 + 自定义；删除「默认语言」；emoji 默认开；精简页面（候选：系统提示词与固定问候收进「高级」，或把固定问候并入「连接与问候」）。
4. 记忆页：待定方向（建议：事实可编辑、导出/导入记忆、按人格查看、事件查看器、隐私说明）。
5. 连接与问候页：新增模型供应商（DeepSeek / 自定义）——DeepSeek 预填 Base URL，填入 API Key 后可拉取模型列表（需要新的网络命令 + 异步 UI 状态；自定义走 OpenAI 兼容端点，`thinking` 字段必须按供应商门控）。

遗留待办（未变）：macOS 编译与门禁（`bash scripts/verify-macos-all.sh`）、`MACOS_VERIFICATION.md` / `FEATURE_PARITY.md` 收尾。

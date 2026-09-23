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
| REQ-S08 macOS 同构 | 已实现：`SettingsView.swift` 删除全部死字段与三个「保存」按钮，改为 `personaSignature` / `deepSeekSignature` / `memorySignature` / `greetingSignature` + 450 ms 去抖的即时生效；分区改名为「模型服务」「系统」；新增空闲问候四项；系统页新增数据目录与关于区；`EngineClient.updateGreetingConfig` 已就位。macOS 用原生 `Form`/`Section` 分组 | 通过：2026-09-22 `verify-macos-all.sh`，见 E-28d | Mac 人工设置页检查仍待 |
| REQ-S09 门禁 | 通过：Windows `verify-windows.ps1 -Full`（历史见 E-S02/E-S06）；macOS `verify-macos-all.sh` exit 0、XCTest 14/14、native smoke 7/7（E-28d） | E-S02/E-S06/E-28d | Windows W22/W23 与 macOS 设置页视觉/即时生效仍待人工 |
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
| E-S08 | 失败记录（REQ-S13 首轮） | ① XAML 删控件后 code-behind 仍引用 `PersonaDescriptionBox` / `PersonaVerbosityCombo` / `PersonaLanguageBox`（CS0103 + XAML WMC9999）→ 清掉构造函数的即时生效挂接；② `PersonaTonePresetCombo` 同时被 XAML 与构造函数挂事件 → 只保留 XAML 一处。两次失败均保留，修复后 exit 0 |
| E-S09 | REQ-S13 提示词改写 | `cargo test -p petsona-core` **61/61**：新增 `traits_default_to_emoji_enabled`，`effective_prompt_includes_traits_and_pet` 增加「必须含『用与用户相同的语言回答』、不得含『默认语言』」断言 |
| E-S07 | 桌面验收包刷新 | `scripts\package-windows.ps1` exit 0；`Desktop\Petsona-验收-修复版\Petsona-windows-x64-0.1.0\` 已替换为新构建（59 文件，`petsona_ffi.dll` md5 `5a9ec34922d856fe6593706a4bef6a77`），旧包改名保留 |

## 审查及关闭

| REV/优先级 | REQ | 位置/触发/影响 | 必需修复与测试 | 关闭证据/状态 |
|---|---|---|---|---|
| — | — | 尚无审查项（审查对话未开始） | — | — |

## 交付与接续

- 实际完成范围：REQ-S01～S08 实现；REQ-S09 Windows `-Full` 与 macOS 完整门禁均有通过证据；REQ-S10 的计划/执行记录和 Mac 验收文档已更新，功能对照表仍需整体收尾。
- 未完成/失败/SKIP/人工待验：Windows W22/W23 设置视觉；macOS 设置页逐页、即时生效、缩放/模型/记忆体验人工检查；`FEATURE_PARITY.md` 全表状态收尾。
- 对用户数据的影响：`persona.json` / `config.json` 不再写出已删字段，旧文件仍可加载；新增 `UpdateGreetingConfig`（ABI 命令值 36，追加式，ABI 版本仍为 3）。
- 下一个执行者的安全接续点：Mac 自动门禁已通过（E-28d），接续 macOS 设置页实机验收；Windows 侧复核 W22/W23。
- Git 操作是否发生（默认无）：无。
- 完成判定：**未完成**（两端仍有设置页人工项，功能状态汇总文档未完全收尾）。

### 第二批进度（v1.1）

- **REQ-S17 已完成**（2026-09-21）：`SettingsWindow.xaml` 侧边栏六个分区各加 `SymbolIcon`（Folder / Font / Contact / Clock / World / Setting），`PaneDisplayMode="Auto"` + `CompactModeThresholdWidth=720` + `ExpandedModeThresholdWidth=1000`，窗口变窄自动收成纯图标栏。证据：`verify-windows.ps1` exit 0（cargo 95 项 + dotnet 36/36 + FFI 守卫），桌面验收包已刷新为含图标的构建（`petsona_ffi.dll` md5 `2cd17c66b3a843186f87b017942349ee`）。
- **REQ-S11 / S13 / S14 已实施**（2026-09-21 第三轮）：
  - 共享层：`Persona` 删除 `description`，`PersonaTraits` 删除 `language`；提示词不再输出「默认语言：xx」，改为「用与用户相同的语言回答」；`emoji` 默认改为 **true**（只影响新建人格，老文件保持原值）。`PersonaPatch` 同步收缩，旧 `persona.json` 仍可加载。
  - Windows：宠物页两个导入按钮合并为「导入…」（MenuFlyout：文件夹 / zip），「重新扫描」移入 Codex 卡片；人格页删除 简介 / 回答长度 / 默认语言，新增 **语气预设 + 自定义**（预设同时决定 `verbosity`），系统提示词收进「高级」Expander；「固定问候文案」移到「连接与问候」的空闲问候区。
  - macOS：同一套结构（`DisclosureGroup("高级")` + 预设 Picker + 问候文案移入问候 Section + 单一「导入…」按钮 + Codex 区加导入/重扫）。**未编译**，需 Mac 门禁。
  - 证据：`cargo test` core **61/61**、runtime **8/8**；`verify-windows.ps1` exit 0（cargo 95 项 + dotnet 36/36 + FFI 守卫）；失败记录见下。桌面验收包已刷新（`petsona_ffi.dll` md5 `77d58c4caba78e0c5899817f4375c1b9`）。
- **REQ-S12 已实施**（2026-09-22 第四轮）：
  - Windows：`ScaleCombo` → `Slider`（0.5–2.0、`StepFrequency=0.25` + `SnapsTo=StepValues`，即原先 7 个档位吸附），右侧实时显示百分比；拖动即生效（档位离散，最多 6 次写入），落盘仍由 `SetScale` 负责。
  - macOS：设置页「大小」由 Picker 改为 `Slider(step: 0.25)` + 百分比标签；状态栏菜单保留同一套 7 档并**给当前档位打勾**，与滑块完全一致（Windows 托盘菜单本来就没有缩放入口，无需改）。
  - 证据：`verify-windows.ps1` exit 0；`windows-smoke.ps1` **25/25 PASS**（含 N12/N19 空库首启设置窗）。
  - **失败记录（重要）**：首次 smoke 出现 N12/N19 FAIL —— 空库首启时设置窗根本没显示。用隔离探针（空 home + 枚举窗口）复现：`WinUIDesktopWin32WindowClass` 窗口存在但 `visible=False`。根因是 `Slider` 在 `InitializeComponent()` 期间设置 `Minimum/Maximum` 就触发了 `ValueChanged`，此时 `ScaleValueText` 还没构造 → `NullReferenceException` 让设置窗构造失败（异常被上层吞掉，进程仍活着）。修法：处理函数里 `if (_suppressEvents || ScaleValueText is null || ScaleSlider is null) return;`。修复后 smoke 25/25。
- **REQ-S15 已实施**（2026-09-22 第五轮）：
  - 共享层：`PetMemory` 新增 `update_fact`（原地编辑保留 `createdAt`）、`clear_facts` / `clear_events`（分级清空）、`export_persona` / `import_persona`（可移植 JSON：`kind = petsona.memory.persona` + facts/events；导入覆盖当前人格的偏好与事件、保留 last seen / last greeting，外来文件被拒）。
  - ABI：追加 4 个命令（37 `UpdateMemoryFact`、38 `ClearMemoryScope`（value: 0 全部 / 1 偏好 / 2 事件）、39 `ExportMemory`、40 `ImportMemory`），`contracts/petsona.h` 与 `ABI.md` 同步；ABI 版本不变。
  - Windows：记忆页选中一条偏好即填入编辑框、按钮变「更新」（或点「取消编辑」），新增「只清偏好 / 只清事件 / 全部清空」与「导出记忆… / 导入记忆…」（导入前 `ContentDialog` 确认会覆盖）。
  - macOS：同一套（编辑/更新 + 三个分级清空 + 导出/导入面板 + 覆盖确认），`EngineClient` 增加 `updateMemoryFact(id:key:value:)`、`clearMemory(scope:)`、`exportMemory`、`importMemory`。**未编译**，待 Mac 门禁。
  - 证据：`cargo test` core **63/63**（新增 `edit_and_scoped_clears_keep_the_other_half`、`export_then_import_round_trips_one_persona`）、runtime **9/9**（新增 `memory_edit_scoped_clear_and_export_round_trip`）；`verify-windows.ps1` exit 0（dotnet **37/37**，新增 `MemoryFactEditScopedClearAndExportRoundTrip` 覆盖 FFI 全链路）；`windows-smoke.ps1` **25/25 PASS**；桌面验收包已刷新（md5 `75cedaa029714f6fc8019c758274d726`）。
- **REQ-S16a 已实施**（2026-09-22 第六轮，S16 的第一半）：模型供应商 + `thinking` 门控 + 凭据按供应商隔离
  - 共享层：`DeepSeekConfig` 新增 `provider`（`deepseek` / `custom`，默认 deepseek，未知值归一为 deepseek，旧配置自动视为 DeepSeek）；`DeepSeekClient::request_body` 抽出为可测函数，**`thinking` 字段只在 DeepSeek 下发送**（严格 OpenAI 兼容端点会拒绝未知字段）；凭据槽位 `provider/deepseek`、`provider/custom`，DeepSeek 仍回落到旧的 `deepseek` 槽（老用户密钥不丢），`save_api_key(provider, key)` 空 key = 删除。
  - 运行时：`UpdateDeepSeekConfig` 归一化 provider；`SaveDeepSeekKey` 按当前 provider 读写凭据（切换供应商不再互相覆盖）。
  - Windows：连接页新增「服务商」卡片（DeepSeek / 自定义）；选 DeepSeek 时 Base URL 固定为内置地址并只读、显示「禁用思考模式」开关；选自定义时 Base URL 可编辑、隐藏该开关并说明"不会发送 thinking 字段"。
  - macOS：同一套（Picker + Base URL disabled + 自定义说明 + 按 provider 读写 Keychain；`KeychainService.account(for:)` / `hasKey(provider:)` / `deleteDeepSeekKey(provider:)`）。**未编译**，待 Mac 门禁。
  - 证据：`cargo test` core **65/65**（新增 `thinking_field_is_deepseek_only`、`api_key_slots_are_per_provider`）；`verify-windows.ps1` exit 0（dotnet **37/37**，DeepSeek 配置往返断言加入 `provider: custom`）；`windows-smoke.ps1` **25/25 PASS**；桌面验收包已刷新（md5 `86a8f054b45c60c33829ec0aff62ed37`）。
  - 失败记录：`save_api_key` 改签名后冻结的旧 egui 入口编译失败（E0061）→ 旧线调用点固定传 `"deepseek"`，不改其行为。
- **REQ-S16b 已实施**（2026-09-22 第七轮，S16 完成）：拉取模型列表
  - 共享层：`DeepSeekClient::list_models()`（`GET {base}/models`，Bearer 认证）+ 纯函数 `parse_model_ids`（排序 / 去重 / 容忍异常结构）。
  - ABI：文本字段 17 `Models`（JSON 数组）+ 命令 41 `ListModels`；`contracts/petsona.h`、`ABI.md`、FFI 枚举 / 分发 / render 映射同步。
  - 运行时：`ListModels` → 独立线程请求 → `ModelsResult` 回写 `runtime.models` 并发布；进行中忽略重复点击；成功/空列表/失败三种状态都写进状态条（失败提示"可手动填写模型名"）。
  - Windows：模型卡片新增「拉取模型列表」按钮 + 拉取成功后的模型下拉（选中即填入文本框，仍可手填）。
  - macOS：同一套（按钮 + Picker，来自 `PETSONA_TEXT_MODELS`）。**未编译**，待 Mac 门禁。
  - 证据：`cargo test` core **66/66**（新增 `parses_and_sorts_model_ids`）、runtime **10/10**（新增 `list_models_publishes_the_provider_catalog` —— 用本地 stub HTTP 服务验证请求打到 `{base}/models` 并把结果发布到投影）；`verify-windows.ps1 -Full` **exit 0**（cargo 106 项 + dotnet 37/37 + FFI 守卫 + native smoke 25/25 ×2 + 打包结构）；桌面验收包已刷新（md5 `d9927c1fe5407372593c8add4445f762`）。

## 第八轮：W19 反馈修复（2026-09-22）

用户实机复测：W15/W16/W17/W18 通过；**W19 失败**（"API key 清除之后未生效"）。另提三条建议：密钥应显示是否已配置、偏好提取把问句当陈述（"我叫什么" → 记住"称呼=什么"）、响应式布局（侧边栏展开太晚 + 卡片宽度不一）。

| 问题 | 根因 | 修复 |
|---|---|---|
| 清除密钥未生效 | S16a 引入按 provider 隔离的凭据槽位后，`resolve_api_key` 会**依次尝试 `provider/deepseek` 与旧槽 `deepseek`**，而清除只删了新槽：上一轮验收保存在旧槽里的密钥仍然命中，看起来"没清除" | `save_api_key(provider, "")` 现在删除 `key_candidates(provider)` 里的**每一个**槽（DeepSeek = 新槽 + 旧槽；自定义 = 自己的槽）；新增 `key_candidates` 与回归测试 `clearing_covers_every_readable_slot` |
| 用户无法判断是否已配置密钥 | UI 只在本次会话点过保存后才知道状态 | 运行时缓存 `key_configured`（在加载、保存/清除密钥、切换 provider 时刷新，不做每帧 keychain 读取），并把 `keyConfigured` 注入 DeepSeek 配置投影；Windows/macOS 显示"已配置（密钥不会显示）/ 未配置"，未配置时禁用「清除密钥」，密钥框在已配置时提示"输入新密钥可覆盖" |
| 问句被当成偏好 | `extract_preference` 只看前缀（`我叫` + 值），没有区分疑问句 | 含 `?`/`？` 直接不提取；值以疑问词开头（什么/啥/哪/谁/多少/怎么/为什么/what/who/how…）或以 吗/呢/么 结尾也不提取；新增 `questions_are_never_stored_as_preferences` 回归测试（含"我叫什么？""我的名字是什么""我喜欢什么""What is my name?"），并保留"我叫小明"等正例 |
| 侧边栏展开太晚 / 卡片宽度不一致 | 阈值 720/1000 对 125% 缩放的 1000px 窗口偏大；页面容器是 `HorizontalAlignment="Left"`，宽度随内容变化 | 阈值改为 640/900；容器改 `HorizontalAlignment="Stretch"` + `MinWidth=560`（各页卡片对齐，最宽 1000） |

- 证据：`cargo test` core **68/68**（新增 `questions_are_never_stored_as_preferences`、`clearing_covers_every_readable_slot`）；`verify-windows.ps1` exit 0（dotnet 37/37）；`windows-smoke.ps1` 25/25；桌面验收包已刷新（md5 `6543268a804928da9196668b2a5bfa03`）。
- 失败记录：本轮 clippy 先报 `needless_borrow`（`keyring.get(&slot)` → `keyring.get(slot)`），修正后通过。
- 待人工复测：W19（清除密钥后状态变为"未配置"且不再调用模型）、W20（问"我叫什么"不写偏好；"我叫小明"仍写入）、W21（缩到窄窗口看侧边栏收纳 / 拉宽看卡片等宽）。
- 新增人工项：**W20**（偏好提取不把问句当陈述）、**W21**（响应式：640/900 阈值与卡片等宽）。

## 第九轮：卡片顺序 / 失败提示 / 问候归位（2026-09-22）

用户复测：W19 / W20 / W21 全部通过。新反馈：模型拉取失败要有提示、模型服务卡片顺序应为「服务商 → URL → API → 模型 → 高级」、空闲问候移到「外观与交互」、人格与宠物直接绑定（下一轮）、人格与记忆重塑待议。

| 反馈 | 处理 |
|---|---|
| 拉取模型失败要有提示 | 失败信息除状态条外，**就地显示在「模型」卡片内**（红字），由运行时状态文案驱动（`拉取模型列表失败…` / `服务商没有返回…`）；成功时隐藏 |
| 模型服务顺序 | 两端重排为 **服务商 → Base URL → API Key → 模型 → 高级**（超时 / token / 温度 / thinking / 环境变量名收进「高级」） |
| 空闲问候归位 | 从模型页移到**「外观与交互」**页（含固定问候文案、空闲分钟、冷却、最大字数）；导航标签 `连接与问候` → **`模型服务`** |
| 人格与宠物绑定 | **待决策**（数据模型变更，见下一轮提案） |

- 证据：`verify-windows.ps1` exit 0（dotnet 37/37）；`windows-smoke.ps1` **25/25**；桌面验收包已刷新（md5 `c297c8afe24a90bfd7986ed0078f0737`）。
- 待人工：W22（视觉顺序与失败提示）、W23（问候在外观页）。
- macOS 同步做了顺序调整、`高级` 分区、失败提示与问候搬移（**未编译**，待 Mac 门禁）。

## 计划功能面收尾（2026-09-22）

- **settings-consolidation 的功能项 S01–S17 全部实施完毕**（S11–S16 于本轮系列完成，S17 侧边栏图标 / 响应式在此前完成）。
- 仍待办（非功能项）：macOS 自动门禁已于 2026-09-22 通过（E-28d）；Windows W22/W23 与 macOS 设置页人工验收、`FEATURE_PARITY.md` 收尾、Windows 发布项 D1–D6 与 CI 首跑仍待办。

### macOS 设置页最新对齐状态（2026-09-23）

- 早期条目中“macOS 未编译 / 待 Mac 门禁”是历史快照。当前 `SettingsView` 已使用 ScrollView + 卡片 + 自适应控件行，六页顺序与 Windows 一致；旧人格 CRUD 状态已清理。异步 Keychain 启动修复和完整门禁见 E-31，最终设置 Release 编译见 E-32。
- 当前剩余是人工视觉/交互验收：720/900/1000px 窗口的卡片对齐、侧边栏收纳、即时生效、模型失败提示、记忆编辑、Keychain 和 LaunchAgent。另需在 Windows 实体机回归共享 runtime 的异步凭据探针。

## 下一批（用户 2026-09-21 提出的设置页微调，已在计划 v1.1 落盘）

1. 宠物页：合并「导入文件夹 / 导入 zip」为单个「导入…」；「扫描 Codex 宠物」移入 Codex 分区。
2. 外观与交互页：缩放档位改为滑块（需保留托盘菜单一致性 + 吸附档位 + 实时预览）。
3. 人格页：语气改预设 + 自定义；删除「默认语言」；emoji 默认开；精简页面（候选：系统提示词与固定问候收进「高级」，或把固定问候并入「连接与问候」）。
4. 记忆页：待定方向（建议：事实可编辑、导出/导入记忆、按人格查看、事件查看器、隐私说明）。
5. 连接与问候页：新增模型供应商（DeepSeek / 自定义）——DeepSeek 预填 Base URL，填入 API Key 后可拉取模型列表（需要新的网络命令 + 异步 UI 状态；自定义走 OpenAI 兼容端点，`thinking` 字段必须按供应商门控）。

遗留待办（未变）：macOS 编译与门禁（`bash scripts/verify-macos-all.sh`）、`MACOS_VERIFICATION.md` / `FEATURE_PARITY.md` 收尾。

### macOS 门禁状态更新（2026-09-22）

- 上方较早批次中“Mac 未编译 / 无工具链 / 门禁待跑”的状态已被后续工作覆盖。最新共享设置、人格式设置及其测试夹具已在 Mac 上通过完整门禁；先前 E-26f 为历史快照，最新证据见 2026-09-22 后续复核记录 E-28d（core 71/71、FFI 4/4、runtime 14/14、XCTest 14/14、native smoke 7/7）。
- 人工验收仍未完成；此前的“遗留待办”仅保留为历史快照，不再代表当前自动门禁状态。

### 代码审查追修（2026-09-22）

- P1 XCTest 宿主隔离：TestAction pre-action 在宿主启动前创建隔离 home、关闭状态服务并写入假凭据配置；真实用户数据指纹与宿主实际 home 路径由统一门禁验证（E-28d）。
- P2 凭据状态：新增注入式 SecretStore 测试覆盖有 Key / 无 Key；runtime 无 Key 时投影 `keyConfigured=false`，主动问候直接使用固定本地问候；Mac 状态文案覆盖“未配置”。
- P3 设置文案：Mac / Windows 高级提示词说明删除已移除的“语言设置”描述。
- `verify-windows.ps1 -Full` 未在 Mac 环境复跑；共享 runtime 的 no-key 问候分支需 Windows 门禁验证后才可认定双端回归完成。

### macOS 设置标题层级复核（2026-09-23）

- 代码审查指出原生 `NSWindow` 标题与 SwiftUI sidebar/detail 两个 `navigationTitle` 叠加，并由固定内容最小宽度挤压窄窗口。已保留原生窗口标题，将当前页标题移入内容滚动区，并移除内容强制最小宽度。完整实现与门禁证据见 [`native-ui-rewrite` 执行记录 8.25 / E-33b](native-ui-rewrite.md)；720/900/1000pt 实际窗口布局仍待人工确认。

### macOS 26 设置窗原生化（2026-09-23）

- 用户确认 macOS 六页整体按最新原生规范重做，最低版本升至 26。当前 `NSWindow` 以系统 tracking separator 将标题栏与 `NavigationSplitView` 分界对齐；详情使用 grouped `Form`，隐藏顶部滚动模糊；上次查看页可恢复；清理记忆前明确确认。计划契约更新到 v1.2 / REQ-S18，minimum version 及发布说明已同步。
- 自动门禁在 macOS 27.0 arm64 通过：XCTest 22/22，native smoke 7/7，Release `.app` 声明 `LSMinimumSystemVersion=26.0`，包结构、签名和隔离目录通过。详细失败/重跑记录见 [`native-ui-rewrite` 执行记录 8.26 / E-34](native-ui-rewrite.md)。
- 仍待 macOS 27 GUI 人工检查：720/900/1000pt、浅/深色、辅助功能对比设置、标题栏分隔、顶部模糊、工具栏切换、即时生效和危险操作；macOS 26 实机也未在本轮提供。

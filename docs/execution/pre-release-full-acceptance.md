# pre-release-full-acceptance：执行记录

## 接手

- 计划路径/版本：`docs/plans/pre-release-full-acceptance.md` v1.0。
- 开始日期：2026-10-09。
- 当前阶段：FA 前端内容界面；FA-0～FA-5 已关闭；FA-6 连接/系统/聊天自动证据与素材已就绪，待人工验收。
- 环境原则：使用隔离 `PETSONA_HOME`、独立端口；真实 `%APPDATA%\Petsona` 与 Codex 原始宠物目录只读。

## FA 阶段

| 轮次 | 状态 | 自动证据 | 人工结果 | 备注 |
|---|---|---|---|---|
| FA-0 设置项旧版核对 | 已关闭 | 已完成旧 macOS 对照与实现/单测/冒烟 | 2026-10-09 用户通过 | 拖放导入明确 DEFER |
| FA-1 设置窗口/布局 | 已关闭 | 构建、桌壳单测、两套无鼠标冒烟、宽窄截图与关闭语义均通过 | 2026-10-09 用户通过 | 无遗留 |
| FA-2 宠物页 | 已关闭 | 宠物库 9 项 + SelectPet 引擎测试通过；导出对话框缓冲修复 + 4 项回归测试（11 passed） | 2026-10-09 用户：FA-2-1..5、7..9 通过；FA-2-6 修复后复测通过 | 修复已提交 `56deb45` |
| FA-3 外观与交互 | 已关闭 | 四项无行为开关已从 UI 下架（配置/命令保留兼容）；前端与 Windows 构建通过 | 2026-10-09 用户复测通过 | v1.1 收缩后关闭；修复已提交 `b1d0538` |
| FA-4 人格页 | 已关闭 | 人格库 13 项 + 运行时人格 4 项测试通过；导入/聊天记录素材与共享 Key 检查就绪 | 2026-10-09 用户通过 | 原生点击交互缺失转入 NA 待办 |
| FA-5 记忆页 | 已关闭 | 核心记忆 15 项 + 运行时 2 项测试通过；隔离目录已播种 2 条 pending 候选与归档事实；导入样例就绪 | 2026-10-09 用户通过 | 无遗留 |
| FA-6 连接/系统/聊天 | 待人工验收 | 流式聊天 8 项 + 模型拉取 1 项测试通过；隔离目录已播种 60 条聊天历史；Key 隔离策略已定 | 待用户 | Key 保存/清除用自定义槽，不动 DeepSeek Key |

## FA-0 证据

| 证据ID | 内容 | 结果 |
|---|---|---|
| E-FA0-01 | 核对旧 macOS 人格/记忆目标 | 已找到 `git show 6bca241:docs/execution/macos-companion-evolution.md`；目标为流式聊天/持久历史/可确认记忆/资料塑造人格，详细语义已写入计划 |
| E-FA0-02 | 设置页响应式修复 | 根因：`.setting-row` 的 grid 右列最小 220px，压缩左侧标签列到接近 0；改为可收缩 flex（标签最小 180px，控制列 `flex: 0 1 520px`）。`tsc --noEmit`、`vite build`、Windows 构建通过；截图 `%TEMP%\petsona-layout-fixed.png` 显示标签恢复横向 |
| E-FA0-03 | 无鼠标冒烟 | exit 0，但该次桌面焦点/动画采样受真实桌面交互干扰（settings foreground=False、distinct frames=1），不作为焦点/动画通过证据；此前同一构建逻辑的干净冒烟见 M4 批次 |
| E-FA0-04 | FA-0-1/2 人格页、外观页 | 人格与模型连接改为 450ms 去抖自动保存；固定问候文案移到外观与交互；API Key 仍显式保存 |
| E-FA0-05 | FA-0-3 Windows MSVC | 新增 `autostart.rs`（HKCU Run，`PETSONA_AUTOSTART_VALUE` 隔离值名）；桌面壳单测 `autostart_uses_an_isolated_value_name` 通过 |
| E-FA0-06 | FA-0-5/6 记忆页 | 归档事实、整理中状态、记忆页“查看聊天记录”入口已接入 |
| E-FA0-07 | 回归 Windows 隔离数据目录 | `desktop-smoke.ps1` EXIT=0；启动 1294–1387ms；焦点/几何/动画/协议/Composer/退出全部通过 |
| E-FA0-08 | FA-0 人工 | 2026-10-09 用户回复“验收通过，继续”，FA-0-1..8 全部关闭（FA-0-4 拖放为明确 DEFER） |

## FA-1 证据（2026-10-09）

| 证据ID | REQ | 环境 | 操作 | 结果 |
|---|---|---|---|---|
| E-FA1-01 | FA-1-1..8 构建 | WSL（Node 24.19.0） | `pnpm build`（`tsc --noEmit` + `vite build`） | exit 0；41 modules；CSS 21.93kB / JS 203.25kB |
| E-FA1-02 | 回归 | Windows MSVC 1.98.0 | `scripts/desktop-build-windows.ps1` | exit 0；debug 构建产出目标 exe |
| E-FA1-03 | 回归 | Windows MSVC 1.98.0 | `cargo test --bin petsona-desktop` | exit 0；7 passed（IPC 字段、空投影、自启隔离值名） |
| E-FA1-04 | FA-1-1 启动路径 | Windows 隔离数据目录 | `scripts/desktop-smoke.ps1`（无鼠标） | EXIT=0；启动 1274/1317/1418ms；位置恢复精确、几何 288x312@96dpi、宠物不抢焦点、编辑条 36x6；`--show-settings` 可见且前台=True；单实例、协议 /health /pets /state TTL/400/clear、Composer、退出释放端口、空库开设置全部通过 |
| E-FA1-05 | FA-1-2/3/6/7（代理） | Windows 隔离数据目录 | `scripts/desktop-settings-smoke.ps1`（无鼠标；窗口重设尺寸 + WM_CLOSE + PrintWindow） | PASS；默认外框 936x639@96dpi（客户区 920x600）；缩至 700 CSS px；WM_CLOSE 后 hidden=True、进程存活、宠物仍可见、/health 正常；ShowWindow 重显后采样色数 55（非白屏） |
| E-FA1-06 | FA-1-2/3 目视 | 同上截图 | `petsona-fa1-wide.png` / `petsona-fa1-narrow.png` | 宽窗口六页“图标+文字”完整；700px 时收纳为仅图标，内容仍可用、无竖排文字 |
| E-FA1-07 | FA-1-1/7/8 代码路径 | WSL/Windows | `present_window`（unminimize→show→focus）覆盖设置与聊天窗口；侧栏按钮补 `aria-label`；新增 `:focus-visible` 轮廓 | 编译通过；托盘重开、键盘可见焦点、主题热切换仍需人工确认 |
| E-FA1-08 | FA-1 人工 | 2026-10-09 用户回复“验收通过，继续下一阶段” | FA-1-1..8 全部关闭 |

命令与产物：

- 截图：`%TEMP%\petsona-fa1-wide.png`、`%TEMP%\petsona-fa1-narrow.png`、`%TEMP%\petsona-fa1-reopen.png`（本机 `C:\Users\happyddz\AppData\Local\Temp\`）。
- `desktop-settings-smoke.ps1` 以 `PETSONA_HOME=%TEMP%\petsona-settings-smoke`、端口 17899 运行；结束时只清理本次进程。

软检查说明：本次 `desktop-smoke.ps1` 的 `state after TTL` 打印为 `look-row-9` 而非 `idle`；当时光标位于注视触发区，/health 返回的是注视姿势，脚本未判定失败。注视观感在 NA 轮单独人工验收。

### FA-1 待人工验收清单

1. FA-1-1：右键托盘图标 → “设置…”；窗口出现并可直接键盘操作（前台聚焦）。
2. FA-1-2：默认宽度下六个导航项文字与图标都完整（对照 `petsona-fa1-wide.png`）。
3. FA-1-3：把窗口拖窄到约 780px 以下，侧栏收纳为仅图标，页面仍可用（对照 `petsona-fa1-narrow.png`）。
4. FA-1-4：逐页滚动到底，卡片、按钮与底部操作不被裁切、无横向滚动。
5. FA-1-5：窗口开着时在 Windows“个性化 → 颜色 → 选择模式”切换浅色/深色；WebView 内容与标题栏都立即跟随，无需重开。
6. FA-1-6：点设置窗口的 ✕；窗口消失但宠物、聊天与进程继续运行（托盘还在）。
7. FA-1-7：再从托盘打开设置；直接回到可用界面，无白屏/错误兜底。
8. FA-1-8：只用键盘 Tab/Shift+Tab/Enter/Space 遍历控件；焦点轮廓可见，按钮可激活，无焦点死循环。

## FA-2 证据（2026-10-09）

| 证据ID | REQ | 环境 | 操作 | 结果 |
|---|---|---|---|---|
| E-FA2-01 | FA-2-1/3/6/7 底层 | WSL | `cargo test -p petsona-core pet::library` | exit 0；9 passed（发现/扫描、导入去重、目录与 ZIP 身份读取、ZIP 往返、路径穿越拒绝、条目上限） |
| E-FA2-02 | FA-2-2/3 切换 | WSL | `cargo test -p petsona-runtime --lib copying_a_persona_to_another_pet` | exit 0；1 passed（SelectPet 切换两只宠物后绑定跟随、切回恢复） |
| E-FA2-03 | FA-2 隔离环境 | Windows | `PETSONA_HOME=%TEMP%\petsona-preaccept`（Boba + Rocky，V2 webp，端口 17920） | 已就绪 |
| E-FA2-04 | FA-2-4/5 导入素材 | Windows | `%TEMP%\petsona-fa2`：`TestPetC`（文件夹，id testpetc）、`import-clean.zip`（id testpetd）、`import-conflict.zip` 与 `BobaConflict`（id boba，同 ID 冲突） | 已就绪 |
| E-FA2-05 | FA-2-6 根因与修复 | WSL + Windows MSVC | `dialog.rs::save_file_dialog` 原来用 `Vec<u16>` 只装默认名再 `reserve`，`buffer.iter()` 扫描范围止于默认名长度；`GetSaveFileNameW` 写入的绝对路径落在 `len` 之外，读取得到 `None`（或默认名），前端 `if (path)` 直接跳过 → 无文件、无提示。改为 `dialog_buffer(initial, 32768)` 零填充整块缓冲 + `path_from_buffer` 全长度扫描；新增 4 项 Windows-only 回归测试 | `cargo test` exit 0；**11 passed**；重建 exe 完成 |
| E-FA2-06 | FA-2-6 复测 | Windows 隔离目录 | 用 `%TEMP%\petsona-preaccept` 重跑：选中 Rocky → 导出到 `%TEMP%\petsona-fa2\export-rocky.zip`；检查文件存在且含 `pet.json` + `spritesheet.webp`；状态行显示“已导出 <绝对路径>” | 待用户复测 |
| E-FA2-07 | FA-2 人工 | 2026-10-09 用户回复“验收通过，继续下一阶段” | FA-2-1..9 全部关闭；导出修复复测通过 |

说明：FA-2-6 的导出修复改动 `apps/desktop/src-tauri/src/dialog.rs`，同时覆盖人格导出（同一保存对话框）；导入/冲突/覆盖语义复用 runtime `ImportPet` 既有实现。

### FA-2-6 复测步骤

1. 退出旧实例（托盘 → 退出），用同一隔离目录重启：`$env:PETSONA_HOME = "$env:TEMP\petsona-preaccept"`。
2. 宠物页选中 Rocky → “导出” → 保存为 `%TEMP%\petsona-fa2\export-rocky.zip`。
3. 确认三件事：文件存在且约 2.4MB；压缩包内含 `pet.json` 与 `spritesheet.webp`；页面状态行显示“已导出 C:\...\export-rocky.zip”（绝对路径）。

### FA-2 待人工验收清单

前置：本地库已有 Boba（当前）与 Rocky；`%TEMP%\petsona-fa2` 已备好导入包。

1. FA-2-1：打开设置 → 宠物页；当前宠物卡显示 Boba 与首帧缩略图；本地列表显示 Boba、Rocky（缩略图 + V2 标签）。
2. FA-2-2：单击 Rocky —— 仅改变选择（底部显示“已选择：Rocky”），当前宠物仍是 Boba，桌面宠物不切换。
3. FA-2-3：双击 Rocky —— 切换为当前宠物；活动卡片、行内“当前”徽章与桌面宠物同步；再双击 Boba 切回。
4. FA-2-4：“导入”菜单分别测试：
   - 选择宠物文件夹 → `%TEMP%\petsona-fa2\TestPetC`；
   - 选择 ZIP 文件 → `%TEMP%\petsona-fa2\import-clean.zip`。
   导入成功后新宠物出现在列表并成为当前宠物。
5. FA-2-5：导入 `%TEMP%\petsona-fa2\import-conflict.zip`（id 与本地 Boba 相同）→ 出现“发现同 ID 宠物”确认框；点“取消”本地不变；再次导入后点“覆盖导入” → Boba 被替换，列表与当前状态正常。
6. FA-2-6：选中 Rocky → “导出” → 保存到 `%TEMP%\petsona-fa2\export-rocky.zip`；打开压缩包确认包含 `pet.json` 与 `spritesheet.webp`。
7. FA-2-7：选中 TestPetC → “删除” → 有确认框；确认后从列表消失。若删除的是当前宠物，应自动切换到另一只且不崩溃。
8. FA-2-8：点“扫描 Codex 宠物” → 出现 Boba、Rocky 候选且候选有缩略图（来源 `%USERPROFILE%\.codex\pets`）。
9. FA-2-9：先选中本地 Rocky → 删除；再双击 Codex 候选里的 Rocky → 导入到本地库并成为当前宠物；确认 `%USERPROFILE%\.codex\pets\rocky` 仍然存在（Codex 原始目录未被修改）。

## FA-3 证据（2026-10-09）

| 证据ID | REQ | 环境 | 操作 | 结果 |
|---|---|---|---|---|
| E-FA3-01 | FA-3-1..6 底层 | WSL | `cargo test -p petsona-runtime --lib appearance_and_greeting_settings_apply_and_survive_a_restart` | exit 0；新增回归测试覆盖：显隐/穿透/缩放/活动提醒/重力/置顶即时进入投影；问候（启用、7 分钟、冷却 33、64 字）与聊天历史开关写入 config；重启后全部恢复 |
| E-FA3-02 | 回归 | WSL（放行回环） | `cargo test -p petsona-runtime` | exit 0；**31 passed / 0 failed**（沙箱内 6 项回环测试为环境性失败，放行后全绿） |
| E-FA3-03 | 环境 | Windows 隔离目录 | `%TEMP%\petsona-preaccept`（scale 0.5、穿透开、置顶开、重力关、活动提醒开、问候 30/120/40、端口 17920） | 已就绪，使用 FA-2 复测的同一构建 |
| E-FA3-04 | FA-3 人工复测 | 2026-10-09 用户回复“验收通过，继续下一阶段” | v1.1 收缩后的 6 项复测全部通过 |

失败留档：全量套件有一次并行运行中 `missing_key_projects_unconfigured_and_uses_the_local_greeting` 单次失败；单测复跑与随后两次全量复跑均通过，判定为环境性偶发，非 FA-3 回归。

### FA-3 待人工验收清单（v1.0，其中第 3/4 项按 v1.1 已下架）


前置：用 `%TEMP%\petsona-preaccept` 启动；先关闭“点击穿透”再做拖动类检查。

1. FA-3-1 缩放：拖动“缩放”滑块或点档位按钮 —— 桌面宠物实时改变大小；重启后保持新值。
2. FA-3-2 显隐：关闭“显示宠物” → 宠物立即消失；重新打开 → 立即出现（托盘与设置仍在）。
3. FA-3-3 穿透：关闭“点击穿透” → 可直接点击宠物身体（挥手/气泡）；再打开 → 点击宠物像素落到桌面；切换立即生效、无需重启。
4. FA-3-4 三项：
   - 置顶：关闭后用一个普通窗口盖住宠物区域，宠物应被覆盖；打开后宠物回到最上层。
   - 重力：打开后把宠物拖到屏幕中部松手 → 宠物落到当前显示器工作区底部（任务栏上沿）。
   - 活动提醒：开关切换即可（实际走动间隔默认 45 分钟，本轮只验证即时保存 + 重启保持）。
5. FA-3-5 空闲问候四项：修改启用/空闲分钟/冷却分钟/最大字数（如 1/1/40）→ 无保存按钮、即时写入；重启后数值保持。可选：保持 1 分钟无输入，观察问候气泡（无模型时用固定文案）。
6. FA-3-6 聊天历史：关闭“保存聊天历史” → 重启后仍为关闭；新对话不写入历史的实效在 FA-6 配好 Key 后验证（已存记录不应被删除）。


### FA-3 人工验收发现（2026-10-09，未通过）

用户反馈“部分功能没有实现，例如重力”。代码级核对结果：

| 设置项 | UI 开关 | 配置读写 | 实际行为 | 证据 |
|---|---|---|---|---|
| 缩放 | ✅ | ✅ | ✅ 实时 | `overlay.rs:614-621` 每帧使用 `snapshot.scale` 渲染并调整窗口 |
| 显示宠物 | ✅ | 会话状态（不落盘） | ✅ 实时显隐 | `overlay.rs:558-575` `want_visible` → ShowWindow 切换 |
| 空闲问候 | ✅ | ✅ | ✅ 运行时调度 + 气泡 | 运行时 `greeting_due` 等测试通过 |
| 保存聊天历史 | ✅ | ✅ | ✅ 运行时按 `save_history` 决定持久化 | `engine.rs:1460/2359/2785` |
| 点击穿透（宠物本体） | ✅ | ✅ | ❌ 未接线 | 壳内除 `settings.rs` 投影外无 `click_through` 消费者；像素级透明穿透是独立机制，与开关无关 |
| 始终置顶 | ✅ | ✅ | ❌ 未接线 | 窗口创建即 `WS_EX_TOPMOST`（`overlay.rs:391`），无 `always_on_top` 消费者；关闭开关仍置顶 |
| 重力下落 | ✅ | ✅ | ❌ 无任何实现 | 壳与运行时均无重力逻辑 |
| 活动提醒 | ✅ | ✅ | ❌ 无走动 | 壳无消费者；`session.rs` 的 `walk_*` 字段为死字段（定义后从未读写） |

- 结论更正：`docs/execution/desktop-shell-rust-ts.md` 中 M3-A-05 的“已实现 / 即时生效 / 2026-10-09 通过”不成立——当时仅验证了构建与配置读写，未验证上述四项行为；FA-3-3、FA-3-4 判定未通过。
- 待用户决策（不阻塞其他 FA 轮次）：四项按“实现”还是“从设置页移除/禁用”处理；重力与活动提醒在原 macOS 计划中属明确非目标。

### 处置（2026-10-09，用户拍板）

- 决定：**四项“UI 暴露但行为未实现”的开关全部从设置页移除**（点击穿透、始终置顶、重力下落、活动提醒）。
- 实现：`AppearancePage.tsx` 删除四项开关与过期说明；卡片“显示与交互”收为“显示”（仅保留已实现的显示/隐藏）。
- 兼容：`config.json` 的 `clickThrough` / `alwaysOnTop` / `gravityEnabled` / `autoWalk` 字段、runtime 命令与快照投影**保留**，旧配置可继续加载；后续版本实现行为后再恢复入口。
- 回归测试保留：运行时 `appearance_and_greeting_settings_apply_and_survive_a_restart` 继续覆盖数据层读写与重启恢复。

### FA-3 复测清单（v1.1 收缩后）

1. 外观页不再出现“点击穿透 / 始终置顶 / 重力下落 / 活动提醒”四个开关，也没有相关说明文案。
2. 缩放：拖动滑块/点档位 → 宠物实时变化；重启后保持。
3. 显示宠物：关闭立即隐藏、打开立即出现；托盘与设置不受影响。
4. 空闲问候：启用/空闲分钟/冷却分钟/字数可改，无保存按钮；重启后保持。
5. 聊天历史：开关可切换并重启保持（“新对话不写入历史”的实效留 FA-6 配好 Key 后验证）。
6. 页面滚动与布局正常，无残留空卡片。

## FA-4 证据（2026-10-09）

| 证据ID | REQ | 环境 | 操作 | 结果 |
|---|---|---|---|---|
| E-FA4-01 | FA-4-1/2/3/4/6/7 数据层 | WSL（放行回环） | `cargo test -p petsona-core persona` | exit 0；13 passed（人格库导入/导出往返、模板校验、TXT/JSON 解析、生成 stub 目标样本） |
| E-FA4-02 | FA-4-3/4/5/9 数据层 | WSL | `cargo test -p petsona-runtime --lib persona` | exit 0；4 passed（复制到其他宠物生成独立绑定、首载绑定、导入保持稳定身份、生成上下文只含参考说话人样本） |
| E-FA4-03 | FA-4-5/6/9 环境 | Windows | Windows 凭据管理器只读检查：`com.petsona.desktop` 存在 `provider/deepseek` | 隔离实例可复用该 Key 做草稿生成与试聊；本轮不写入/不清除 Key |
| E-FA4-04 | FA-4-4/6/7 素材 | Windows | `%TEMP%\petsona-fa4`：`persona-import.json`（新 id `fa4-import-style`，二次导入触发同 ID 冲突）、`source-chat.txt`（7 条，目标说话人“阿明”4 条）、`source-chat.json` | 已就绪 |
| E-FA4-05 | FA-4 人工 | 2026-10-09 用户回复“验收通过” | FA-4-1..9 全部关闭 |

### FA-4 待人工验收清单

前置：用 `%TEMP%\petsona-preaccept` 启动（当前宠物 Boba，本地另有 Rocky）。第 5/6/9 项需要模型，使用共享 DeepSeek Key，不会修改 Key。

1. FA-4-1：人格页修改语气（点预设或自定义输入）、emoji、系统提示词 → 显示“自动保存”；重启窗口后保持。
2. FA-4-2：点“重置为内置” → 有确认框；确认后回到内置默认风格；切到记忆页确认长期事实不变。
3. FA-4-3：点“复制到…” → 选择 Rocky；到宠物页双击切换到 Rocky，看到独立副本（当前宠物自定义）；切回 Boba，原风格不变。
4. FA-4-4 导入/导出：
   - 导出：保存为 `%TEMP%\petsona-fa4\export-persona.json`，确认包含 id/name/traits/systemPrompt；
   - 导入：选择 `%TEMP%\petsona-fa4\persona-import.json` → 成功并切换为“验收导入风格”；
   - 再次导入同一文件 → 出现“人格 ID 已存在”确认框 → 先“取消”（本地不变），再“覆盖导入”（成功）。
5. FA-4-5：“从资料学习说话方式”卡片选“人物描述” → 输入名字和描述 → 生成草稿；草稿字段可编辑。
6. FA-4-6：粘贴 `source-chat.txt` 全文 → 解析 → 选择目标说话人“阿明” → 显示消息数与预览；再粘贴 `source-chat.json` 验证同样解析。
7. FA-4-7：用“选择 TXT / JSON 文件”分别选择 `source-chat.txt` 与 `source-chat.json` → 解析正常。
8. FA-4-8：草稿存在时点“放弃草稿” → 草稿消失；当前人格与宠物记忆不变。
9. FA-4-9：草稿卡片“试聊”输入一句 → 回复符合草稿风格；聊天历史不新增该轮、记忆页事实不变。

## FA-5 证据（2026-10-09）

| 证据ID | REQ | 环境 | 操作 | 结果 |
|---|---|---|---|---|
| E-FA5-01 | FA-5-2/4/5/6/7 数据层 | WSL | `cargo test -p petsona-runtime --lib memory` | exit 0；2 passed（记忆编辑/分级清空/导出往返、问候写入记忆） |
| E-FA5-02 | FA-5-1..8 数据层 | WSL | `cargo test -p petsona-core memory` | exit 0；15 passed（提取/纠正、习惯三次门槛、确认与拒绝不复发、保留期、压缩、导入导出往返、非法文件不覆盖） |
| E-FA5-03 | FA-5-3/4/5 环境 | Windows 隔离目录 | 应用未运行时备份并扩充 `%TEMP%\petsona-preaccept\memory.json` | 现有：事实“称呼=可以叫我小审”、归档“旧偏好=曾喜欢喝冰美式”、候选“饮品偏好”(0.82) + 新增“作息=习惯早睡”(0.66，2 条证据)；备份 `memory.json.fa5-backup` |
| E-FA5-04 | FA-5-7 素材 | Windows | `%TEMP%\petsona-fa5\memory-import.json`（kind `petsona.memory.persona`、1 条导入事实 + 1 条归档） | 已就绪 |
| E-FA5-05 | FA-5 人工 | 2026-10-09 用户回复“验收通过，继续” | FA-5-1..8 全部关闭 |

### FA-5 待人工验收清单

前置：用 `%TEMP%\petsona-preaccept` 启动（Boba 绑定 persona `default`）。

1. FA-5-1：记忆页关闭“记忆”开关 → 状态即时变化；再打开（无保存按钮）。
2. FA-5-3：“待确认的习惯”显示两条候选（饮品偏好 0.82、作息 0.66），每条含可信度与最多两条证据。
3. FA-5-4：确认“饮品偏好” → 候选消失；长期偏好出现“饮品偏好=喜欢美式咖啡”，来源显示“对话”。
4. FA-5-5：忽略“作息” → 候选消失。
5. FA-5-8：重启实例 → 已确认偏好保留；被忽略候选不再出现；归档事实仍带“归档”标签。
6. FA-5-2：新增事实（如“城市=上海”）→ 就地出现；编辑该事实 → 就地更新且不产生重复；删除一条事实 → 列表立即更新。
7. FA-5-7 导出/导入：
   - 导出 → 存 `%TEMP%\petsona-fa5\memory-export.json`，确认含 kind/facts/events 字段；
   - 导入刚才的导出文件 → 先出现覆盖确认框 → 导入后条数与字段一致；
   - 再导入 `%TEMP%\petsona-fa5\memory-import.json` → 出现“称呼=验收导入的小明”与归档“旧偏好”。
8. FA-5-6 分级清空：
   - “清空偏好” → 确认后偏好清空、互动事件仍在；
   - 重新导入 `memory-import.json` 恢复偏好 → “清空事件” → 事件清空、偏好保留；
   - “清空全部” → 偏好与事件均清空。

## NA 待办（原生浮层与输入，FA 后执行）

来源：FA-4 验收期间用户发现“点击宠物无互动动作”。代码核对：

- 旧 C# 语义（`git show 6bca241:apps/windows/Petsona/AppController.cs`）：单击（320ms 防双击冲突）→ `SetState waving` + 气泡“你好，我在这里”(5s)；双击 → `SetState jumping`；拖动每 80ms 重发 running-left/right。
- 新 Rust 壳现状：`overlay.rs::on_lbutton_up` 只记录 `pet: clicked`；全壳从未发送 `SetState`/`ShowBubble` → 单击、双击、拖动状态都未接线。
- 结论：属 NA 轮待实现项，不是 FA-4 回归；runtime 侧 `SetState`/`ShowBubble` 命令与防回绕逻辑已存在并有测试。

NA 待实现/验收清单：

1. 单击宠物（未拖动）→ 320ms 后 waving + 气泡“你好，我在这里”（约 5s 消失）。
2. 双击宠物 → jumping；单击延迟需被双击取消。
3. 拖动宠物 → 按方向播放 running-left/right，松手回 idle；重复拖动不重置帧。
4. 拖动跟手、工作区夹取、位置重启恢复。
5. 注视全方向与近距离边界观感（已实现，待复测）。
6. 透明像素穿透、宠物本体可点、光标保持普通箭头。

## FA-6 证据（2026-10-09）

| 证据ID | REQ | 环境 | 操作 | 结果 |
|---|---|---|---|---|
| E-FA6-01 | FA-6-7/8 数据层 | WSL（放行回环） | `cargo test -p petsona-runtime --lib conversation` | exit 0；8 passed（流式分片/多字节 UTF-8、未终止响应拒绝、中途取消、HTTP 错误细节等） |
| E-FA6-02 | FA-6-2 数据层 | WSL（放行回环） | `cargo test -p petsona-runtime --lib list_models` | exit 0；1 passed（模型目录投影） |
| E-FA6-03 | FA-6-9 环境 | Windows 隔离目录 | 应用未运行时写入 `%TEMP%\petsona-preaccept\conversations\626f6261.json`（boba，60 条：30 用户 + 30 回复，version 1） | 已就绪；分页阈值 50 条，打开时应有“加载更早的记录” |
| E-FA6-04 | FA-6-3 隔离策略 | Windows 凭据库 | 只读检查 `com.petsona.desktop` 存在 `provider/deepseek`；Key 保存/清除用**自定义**服务商（槽 `provider/custom`）验证 | DeepSeek Key 不受影响 |

### FA-6 待人工验收清单

前置：`%TEMP%\petsona-preaccept`（当前 boba，60 条历史）；DeepSeek Key 已配置（共享凭据，只读使用）。

1. FA-6-1：连接页顺序为 服务商 → Base URL → API Key → 模型 → 高级参数；各控件可操作、改动自动保存（Key 单独保存）。
2. FA-6-2 模型拉取：
   - 成功：服务商 DeepSeek → “拉取模型” → 显示“已拉取 N 个模型”，可从列表选择；
   - 失败：切“自定义”，Base URL 填 `http://127.0.0.1:9/v1` → 拉取 → 显示“拉取模型列表失败：…（可手动填写模型名）”；
   - 空结果（可选，默认 SKIP）：代码路径为“服务商没有返回任何模型；请手动填写模型名”，需要本地空响应 stub 才能复现。
3. FA-6-3 Key 保存/清除（**隔离策略**）：
   - 服务商保持“自定义”，输入假 Key `fa6-test-key` → 保存 → 显示已配置密文状态；
   - 点“清除” → 确认框 → 变为未配置；
   - 切回 DeepSeek → 仍显示已配置（证明未动 DeepSeek Key）。
4. FA-6-4 系统页：版本（当前 shell 0.1.0，M6 统一）、架构、构建类型、数据目录/日志路径指向 `%TEMP%\petsona-preaccept`、协议端口 17920 均正确。
5. FA-6-5：点“打开仓库” → 默认浏览器打开仓库；点“复制诊断” → 提示已复制，可粘贴检查。
6. FA-6-6：右键托盘 → “聊天与历史” → 独立聊天窗出现且前台聚焦。
7. FA-6-7：发送一条消息 → 流式显示；生成中点“停止” → 显示“已停止”；点“重试” → 重新流式生成。
8. FA-6-8：Enter 发送、Shift+Enter 换行；中文输入法组合期间回车只上屏、不误发。
9. FA-6-9 历史加载/清空：
   - 打开聊天默认显示最近 50 条，出现“加载更早的记录”；
   - 点击后出现第 1~10 条；
   - 发送新消息后重开聊天/重启应用 → 新消息仍在；
   - 点“清空” → 确认框 → 历史清空，重启后仍为空（记忆页事实不应被删除）。

## 证据与限制

- 自动测试不得移动鼠标或注入输入。
- AI 只准备隔离环境、启动应用、提供验收步骤；鼠标/键盘相关最终结果由用户回复。
- 失败证据应包含：轮次 ID、实际现象、预期、截图/日志路径、是否可稳定复现。

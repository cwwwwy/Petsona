# pre-release-full-acceptance：执行记录

## 接手

- 计划路径/版本：`docs/plans/pre-release-full-acceptance.md` v1.0。
- 开始日期：2026-10-09。
- 当前阶段：FA 前端内容界面；FA-0/FA-1 已关闭；FA-2 人工验收 8/9 通过，导出根因已修复，待 FA-2-6 复测。
- 环境原则：使用隔离 `PETSONA_HOME`、独立端口；真实 `%APPDATA%\Petsona` 与 Codex 原始宠物目录只读。

## FA 阶段

| 轮次 | 状态 | 自动证据 | 人工结果 | 备注 |
|---|---|---|---|---|
| FA-0 设置项旧版核对 | 已关闭 | 已完成旧 macOS 对照与实现/单测/冒烟 | 2026-10-09 用户通过 | 拖放导入明确 DEFER |
| FA-1 设置窗口/布局 | 已关闭 | 构建、桌壳单测、两套无鼠标冒烟、宽窄截图与关闭语义均通过 | 2026-10-09 用户通过 | 无遗留 |
| FA-2 宠物页 | 复测中 | 宠物库 9 项 + SelectPet 引擎测试通过；导出对话框缓冲修复 + 4 项回归测试（11 passed） | 2026-10-09 用户：FA-2-1..5、7..9 通过；FA-2-6 导出未生效 | 导出根因：保存对话框只读了默认名长度内的缓冲；已重建 exe 待复测 |
| FA-3 外观与交互 | 待验收 | 待执行 | 待用户 | — |
| FA-4 人格页 | 待验收 | 待执行 | 待用户 | — |
| FA-5 记忆页 | 待验收 | 待执行 | 待用户 | — |
| FA-6 连接/系统/聊天 | 待验收 | 待执行 | 待用户 | — |

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

## 证据与限制

- 自动测试不得移动鼠标或注入输入。
- AI 只准备隔离环境、启动应用、提供验收步骤；鼠标/键盘相关最终结果由用户回复。
- 失败证据应包含：轮次 ID、实际现象、预期、截图/日志路径、是否可稳定复现。

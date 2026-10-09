# pre-release-full-acceptance：执行记录

## 接手

- 计划路径/版本：`docs/plans/pre-release-full-acceptance.md` v1.0。
- 开始日期：2026-10-09。
- 当前阶段：FA 前端内容界面；FA-0 已关闭，FA-1 自动证据完成，待人工验收。
- 环境原则：使用隔离 `PETSONA_HOME`、独立端口；真实 `%APPDATA%\Petsona` 与 Codex 原始宠物目录只读。

## FA 阶段

| 轮次 | 状态 | 自动证据 | 人工结果 | 备注 |
|---|---|---|---|---|
| FA-0 设置项旧版核对 | 已关闭 | 已完成旧 macOS 对照与实现/单测/冒烟 | 2026-10-09 用户通过 | 拖放导入明确 DEFER |
| FA-1 设置窗口/布局 | 待人工验收 | 构建、桌壳单测、两套无鼠标冒烟、宽窄截图与关闭语义均通过 | 待用户 | 托盘重开、主题热切换、滚动、键盘为人工项 |
| FA-2 宠物页 | 待验收 | 待执行 | 待用户 | — |
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

## 证据与限制

- 自动测试不得移动鼠标或注入输入。
- AI 只准备隔离环境、启动应用、提供验收步骤；鼠标/键盘相关最终结果由用户回复。
- 失败证据应包含：轮次 ID、实际现象、预期、截图/日志路径、是否可稳定复现。

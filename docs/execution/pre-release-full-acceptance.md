# pre-release-full-acceptance：执行记录

## 接手

- 计划路径/版本：`docs/plans/pre-release-full-acceptance.md` v1.0。
- 开始日期：2026-10-09。
- 当前阶段：FA 前端内容界面，尚未开始实际人工结果记录。
- 环境原则：使用隔离 `PETSONA_HOME`、独立端口；真实 `%APPDATA%\Petsona` 与 Codex 原始宠物目录只读。

## FA 阶段

| 轮次 | 状态 | 自动证据 | 人工结果 | 备注 |
|---|---|---|---|---|
| FA-0 设置项旧版核对 | 待人工验收 | 已完成旧 macOS 对照与实现/单测/冒烟 | 待用户 | 拖放导入明确 DEFER |
| FA-1 设置窗口/布局 | 待验收 | 待执行 | 待用户 | 先从前端第一批开始 |
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
| E-FA0-04 | FA-0-1/2 | 人格页 / 外观页 | 人格与模型连接改为 450ms 去抖自动保存；固定问候文案移到外观与交互；API Key 仍显式保存 |
| E-FA0-05 | FA-0-3 | Windows MSVC | 新增 `autostart.rs`（HKCU Run，`PETSONA_AUTOSTART_VALUE` 隔离值名）；桌面壳单测 `autostart_uses_an_isolated_value_name` 通过 |
| E-FA0-06 | FA-0-5/6 | 记忆页 | 归档事实、整理中状态、记忆页“查看聊天记录”入口已接入 |
| E-FA0-07 | 回归 | Windows 隔离数据目录 | `desktop-smoke.ps1` EXIT=0；启动 1294–1387ms；焦点/几何/动画/协议/Composer/退出全部通过 |

## 证据与限制

- 自动测试不得移动鼠标或注入输入。
- AI 只准备隔离环境、启动应用、提供验收步骤；鼠标/键盘相关最终结果由用户回复。
- 失败证据应包含：轮次 ID、实际现象、预期、截图/日志路径、是否可稳定复现。

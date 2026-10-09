# settings-ui-deep-review：执行与审查记录

## 接手

- 计划路径/版本：`docs/plans/settings-ui-deep-review.md` v0.1（草案，未批准执行）。
- 用户执行授权 / 日期：**未授权执行**。2026-10-09 用户明确「刚刚你发的这些先不要执行，写成一个文档或者计划，
  我先休息一会，等会再开始」；本轮仅授权落盘文档与记录审查证据。
- HEAD、分支、相关 dirty/untracked：HEAD `3ccd819`；工作区 dirty 为 DI 批次 12 个文件（见
  `pre-release-full-acceptance.md` 的 DI 段）；本轮新增计划与执行文档；`.scratch/ui-review/` 截图被 git 忽略。
- 关键目标与验收复述：深度复查设置六页的设置项、元素、控件、间距与 UI，先形成文档；执行与验收标准见计划
  REQ-S1…S4、T-01…T-04、H-01…H-03。
- 本次实际状态：**审查已完成（只读），实现未开始**。

## 要求与文件

| REQ | 改动文件与具体行为 | 实现状态 | 自动验证 | 人工验收 | 剩余工作 |
|---|---|---|---|---|---|
| REQ-S1-1…S1-5 | `app.css` / `ui.tsx` / `App.tsx`：纵向行 flex、滑块 margin、滚动条稳定、死代码、切页滚动 | 未实现 | 未跑 | 未做 | 待 D1–D5 拍板后执行 |
| REQ-S2-1…S2-4 | 人格「表达」卡、系统页去术语、连接页文案、记忆删除按钮/草稿卡色/提示词状态 | 未实现 | 未跑 | 未做 | 依赖拍板 |
| REQ-S3-1…S3-3 | 确认对话框统一、页头收敛、可达性与对比度 | 未实现 | 未跑 | 未做 | 依赖拍板 |
| REQ-S4-1 | 弹层外点关闭、事实列表、toast、blur 性能 | 未实现 | 未跑 | 未做 | 可延后 |

## 审查证据（追加，不覆盖失败历史）

| 证据ID/时间 | REQ与代码版本 | cwd/目标入口 | 命令/方法 | 退出码/结果 | 结果路径及限制 |
|---|---|---|---|---|---|
| E-SR-01 / 2026-10-09 | 计划基线（HEAD `3ccd819` + DI dirty） | `/home/cwwwwy/Petsona` | `PATH="$HOME/.nvm/versions/node/v24.19.0/bin:$PATH" pnpm --dir apps/desktop build` | 退出码 0；CSS 21.20 kB / JS 194.92 kB | `apps/desktop/dist`（已忽略） |
| E-SR-02 / 2026-10-09 | 六页布局审查 | `/tmp/petsona-ui-review` | 模拟 Tauri 桥（`mock.js`）+ 本地 `http.server` + Windows Chrome headless（`--screenshot` / `--dump-dom`） | 六页 × 3 档窗口截图与几何 JSON 全部产出 | 截图副本 `.scratch/ui-review/*.png`；临时工程 `/tmp/petsona-ui-review/{mock.js,shots,metrics}` |
| E-SR-03 / 2026-10-09 | REV-01 | 同上 | `getBoundingClientRect` | 「固定问候文案」行 333px（标签区 flex 高度 220px、内容 86px）；人格「补充说明」行 346px | 见 E-SR-02 几何 JSON |
| E-SR-04 / 2026-10-09 | REV-02 | 同上 | `getComputedStyle(input[type=range])` | UA `margin: 2px`；右缘 开关 1090 / 文本域 1090 / 滑块 1092 | 同上 |
| E-SR-05 / 2026-10-09 | REV-03 | 同上 | 同一视口对比两页 | 连接页控件右缘 1105（无滚动条）vs 外观页 1090（有滚动条），差 15px | 同上 |
| E-SR-06 / 2026-10-09 | REV-13 | 仓库只读扫描 | 类名/组件引用扫描 | `NumberField` 无引用；`.number-field/.section-toolbar/.wrap/.danger-text/.pet-avatar/.pet-thumb-large` 无引用（动态拼接类已排除） | 扫描脚本输出 |
| E-SR-07 / 2026-10-09 | 限制说明 | — | — | 无头 Chromium 与 WebView2 存在细微渲染差异；最终观感需用户实机确认 | 计划已注明 |

## 冲突与变更请求

| CR | 文件事实 | 影响REQ/范围 | 建议 | 用户决定/计划版本 |
|---|---|---|---|---|
| CR-SR-01 | 用户明确「先不要执行」 | 全部 REQ | 保持草案，等待拍板 | 已确认（2026-10-09） |
| CR-SR-02 | DI 批次 12 个文件仍未提交 | 执行基线 | 执行前先提交，避免与 DI 证据混淆 | 待用户 |

## 审查及关闭

| REV/优先级 | REQ | 位置/触发/影响 | 必需修复与测试 | 关闭证据/状态 |
|---|---|---|---|---|
| REV-01…03（P0） | S1 | 见计划 REV 表 | 修复 + T-02 几何回归 + H-01 | 未关闭 |
| REV-04…10（P1） | S2/S3 | 见计划 REV 表 | 拍板后实现 + H-02 | 未关闭 |
| REV-11…20（P2） | S3/S4 | 见计划 REV 表 | 实现 + H-03 | 未关闭 |

## 交付与接续

- 实际完成范围：仅审查与文档落盘（`docs/plans/settings-ui-deep-review.md`、本文件、
  `pre-release-full-acceptance.md` 的 DI 验收更新）。
- 未完成/失败/SKIP/人工待验：全部实现未开始；待用户休息后确认 D1–D5 并授权执行。
- 对用户现有数据/行为的影响：无（未改代码/配置/数据）。
- 下一个执行者的安全接续点：读计划「决定点」表 → 用户拍板 → 从 S1（纯 CSS）开始。
- Git操作是否发生：无（AI 未执行 commit/push）。
- 完成判定及对应证据：未完成；证据见 E-SR-01…07。

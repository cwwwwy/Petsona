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

## 2026-10-10 接续（执行中）

- 用户授权读取文档并推进设置修复、随后 M5；D1–D5 全部采用推荐方案，S4 一起收口。
- HEAD `63dfc9b`，接手工作区干净；CR-SR-01 暂缓决定已更新，CR-SR-02 DI 基线冲突已解除。
- 计划升级 v1.0；Mac arm64 / macOS 27.0.1 / Xcode 已安装 / Node 24.18.0。
- 关键目标：消除堆叠空洞、稳定右缘与切页宽度，精简动作文案，统一确认及表单标签，处理长列表与弹层。
- 约束：不改业务数据与 IPC；不注入输入；Windows 构建/冒烟及人工结果不得由 Mac 模拟证据替代。

### 当前逐 REQ 状态（历史表不覆盖）

| REQ | 实现文件与行为 | 自动验证 | 人工验收/剩余 |
|---|---|---|---|
| S1-1 | app.css堆叠flex复位；问候/补充说明初始文本域紧凑 | 最终最大138px，36组合通过 | H-01待验 |
| S1-2 | range margin=0 | 右缘差0px | H-01待验 |
| S1-3 | content scrollbar-gutter stable | 同视口六页卡片右缘差0px | Windows实际滚动条待验 |
| S1-4 | 删NumberField与既有未用选择器；首行分隔线显式 | 构建、引用扫描通过 | 无业务变更 |
| S1-5 | App useLayoutEffect切页回顶部 | 构建/代码检查；未自动注入切页输入 | 人工切页待验 |
| S2-1 | PersonaPage移除表达卡；保存系统提示词不再提交emoji | 构建/代码检查 | 旧emoji=false保存/导入待验 |
| S2-2 | SystemPage仓库入口唯一、去开发术语、平台中性文案 | 构建/引用扫描 | 文案实机待验 |
| S2-3 | ConnectionPage去顺序评审与config字段枚举 | 构建/引用扫描 | 文案实机待验 |
| S2-4 | 行内删除ghost且hover/focus显示；草稿强调色；提示词默认/自定义徽章 | 构建、模拟草稿截图 | 操作/默认徽章待验；展示比较默认文案，不修改Rust真相 |
| S3-1 | ConfirmDialog/原生HTML dialog；7处confirm替换，复制/覆盖复用Dialog | 构建/引用扫描；未自动注入键盘 | Esc/圈闭/恢复焦点/取消与确认待验 |
| S3-2 | ActionMenu；宠物导入、人格主操作+更多、记忆主操作+更多 | 构建/六页截图 | 外点/Esc/Tab/实际文件对话框待验 |
| S3-3 | SettingRow context/useId与字段aria关联；刻度删除；浅深提示对比度调高 | 36组合无未标记字段/aria-hidden可聚焦后代；正文提示对比度达标 | 辅助技术/键盘实机待验 |
| S4-1 | 卡片无blur；菜单外点/Esc；事实前8条折叠；空候选一行；删除宠物提示保留人格记忆 | 构建、12条模拟事实只显示8条、空态/代码扫描 | 展开收起、候选出现、删除语义待验 |

### 命令与证据（2026-10-10）

所有命令 cwd `/Users/book/Desktop/Petsona`；目标共用TS前端；执行环境macOS27.0.1/arm64，Node24.18.0、pnpm12.10.1。未改lockfile与依赖声明。

| ID | 命令/方法 | 退出码与结果 | 结果位置/限制 |
|---|---|---|---|
| E-SR-08a | `pnpm --dir apps/desktop install --frozen-lockfile` 沙箱 | exit1；DNS无法解析registry、ERR_PNPM_META_FETCH_FAIL；失败保留 | 本机首次无node_modules |
| E-SR-08b | 放行网络安装锁定依赖 | 0；73包安装完成 | 安装产生的临时store已清理，node_modules保留且忽略 |
| E-SR-09 | `pnpm --dir apps/desktop build`（最终代码） | 0；tsc+vite，43模块，CSS20.81kB/JS197.47kB | `apps/desktop/dist`（忽略） |
| E-SR-10a | 无头Chrome沙箱/首轮放行 | 沙箱退出-6；放行轮次超时25s，DOM已产出但Chrome退出卡在系统显示链接 | Chrome stderr CVDisplayLink错误；失败保留 `.scratch/settings-ui-review/chrome-failure.txt` |
| E-SR-10b | 首轮18组合几何 | 0；右缘/标签/溢出通过；随后发现问候152px/补充144.2px超140目标 | 修复初始rows/min-height，不降低验收 |
| E-SR-10c | 浅色对比度轮次 | 1；“当前”徽章嵌套选中背景对比度3.61 | 调整浅色positive/warning与正文提示颜色，失败保留于本记录 |
| E-SR-10d | 最终light/dark无头几何与对比度 | 两轮exit0；各18组合；浅色最低对比度4.78、深色5.31；堆叠最大138px、滑块右缘差0、跨页右缘差0、无横向溢出、字段具名、无隐藏焦点控件 | `.scratch/settings-ui-review/check.py`、`metrics-{light,dark}.json`、六张×两主题截图；36组合 |
| E-SR-11 | `git diff --check`；confirm/死组件/死选择器扫描 | 0；无空白错误、无window.confirm与列举死代码 | Git只读；未提交 |
| E-SR-12 | T-03 Windows构建/设置冒烟 | 未执行（当前只有Mac） | **待验收**；既有 `desktop-settings-smoke.ps1`无鼠标/键盘注入，WM_CLOSE用于生命周期；完整desktop-smoke有输入注入，不在本轮运行 |
| E-SR-13 | T-04 根Rust fmt/clippy/test | 未执行：本批只改TS/CSS，按计划仅跑受影响前端构建 | Mac新壳基线check失败另记M5，不能冒充Windows验证 |

Chrome处理：每组合使用独立临时profile与本地file页面、模拟快照；读到完整metrics后结束仅本轮创建的进程组，避免CVDisplayLink退出挂起；**脚本exit0表示断言通过，不表示Chrome自然退出**。三档实际CSS视口为905×570、1180×820、640×520。主题媒体规则仅在临时副本中固定；未改产品主题跟随逻辑。测量采用计算背景色合成，不包括系统玻璃/渐变采样；WebView2/WKWebView与辅助技术最终结果仍人工确认。

### 交付与阶段门

- S1–S4 已实现；前端构建、模拟布局与对比度通过；**整体待验收**，T-03/H-01～03未关闭。
- 全部人工步骤见 `docs/DESKTOP_VERIFICATION.md` 第5节；确认框、键盘与文件操作不通过自动输入验证。
- M5已完成调查与契约细化，基线编译失败记录在 `desktop-m5-macos.md`；产品实施待设置收口后开始。
- 没有运行用户Petsona实例、读写真实Keychain/LaunchAgent、执行Git写操作。

## 2026-10-10 Windows 侧自动验收（T-03）

环境：Windows 11 / x86_64-pc-windows-msvc 1.98.0；WSL 侧 Node 24.19.0 构建前端；
所有运行使用隔离 `PETSONA_HOME` 与独立端口，未启动用户默认实例，未注入鼠标/键盘。

| ID | 命令/方法 | 退出码与结果 | 结果位置/限制 |
|---|---|---|---|
| E-SR-14 | `PATH="$HOME/.nvm/versions/node/v24.19.0/bin:$PATH" pnpm --dir apps/desktop build` | 0；43 模块；CSS 20.81 kB / JS 197.47 kB（与 Mac E-SR-09 一致） | `apps/desktop/dist`（忽略） |
| E-SR-15 | `scripts/desktop-build-windows.ps1`（debug，MSVC） | 0；`Finished dev profile in 6.42s` | `%USERPROFILE%\petsona-build\desktop-target\debug\petsona-desktop.exe` |
| E-SR-16 | `scripts/desktop-settings-smoke.ps1`（隔离 home `%TEMP%\petsona-settings-smoke`，端口 17899，无鼠标/无输入） | **PASS**：设置窗可见且前台 True；默认 936×639 physical；700px 窄窗缩放成功；WM_CLOSE 后仅隐藏（进程/宠物/`/health` 存活）；重显颜色采样 56（≥20，无白屏） | `%TEMP%\petsona-fa1-{wide,narrow,reopen}.png`；结尾中文提示在控制台代码页下乱码，不影响断言 |

备注：诊断期间发现运行环境自带 `RUST_LOG=warn` 会把 runtime 的 info 级文件日志过滤掉（日志文件 0 字节）；
这是调用环境变量所致，不是产品缺陷——用户自行启动（无该变量）时日志正常，如需调试可显式 `$env:RUST_LOG='info'`。

### 人工验收环境（H-01～H-03，待用户执行）

- 目录：`%TEMP%\petsona-settings-accept`（从既有验收目录复制，端口 **17921**，`clickThrough=false`，`scale=1.0`，位置回到默认）。
- 数据：2 只宠物（boba / song-xiaoxuan）；**10 条长期偏好 + 1 条归档 + 1 条 pending 候选**；4 条聊天记录（用于清空历史确认框）；
  人格 `default.json` 预置 `traits.emoji=false`（验证编辑系统提示词不会重置该字段）。
- 凭据：`provider=custom` + `apiKeyEnv=PETSONA_ACCEPT_KEY`（启动时给出测试值），Key 的保存/清除只会落在 `custom` 槽位，
  不触碰用户真实的 `deepseek` 凭据。
- 启动命令：
  `$env:PETSONA_HOME="$env:TEMP\petsona-settings-accept"; $env:PETSONA_ACCEPT_KEY='sk-accept-test'; & "$env:USERPROFILE\petsona-build\desktop-target\debug\petsona-desktop.exe" --show-settings`
- 清单：`docs/DESKTOP_VERIFICATION.md` 第 5 节 1～7 项；全部为鼠标/键盘操作，由用户手动执行。
- 状态：T-03 自动部分关闭；**H-01～H-03 待人工验收**，未通过前不进入 M5-A 的产品实施。

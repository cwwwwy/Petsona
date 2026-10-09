# settings-ui-deep-review：执行契约（草案，未批准执行）

> 本文件是 2026-10-09 设置六页深度审查的落盘版本。用户明确「刚刚你发的这些先不要执行」；
> 只有用户拍板（并确认 D1–D5 决定点）后才能进入执行，执行者不得自行缩减或跳过验收。

## 身份与授权

- 任务ID / 版本 / 日期：`settings-ui-deep-review` / v0.1 草案 / 2026-10-09。
- 用户目标：M6 打包前把设置页彻底收口——复查每一个设置项、元素、控件、间距与 UI 细节，
  找出尚未考虑到的问题；先形成文档，用户休息后再决定是否执行。
- 已确认决定：
  - DI 批次 5 项人工验收通过（2026-10-09 用户回复），记录见 `../execution/pre-release-full-acceptance.md`。
  - 上一轮回复中提出的 S1–S4 批次「先不要执行」；本计划仅落盘，不含任何代码改动。
- 本次范围：设置六页（宠物 / 外观与交互 / 人格 / 记忆 / 连接 / 系统）的 TS/React 内容界面、
  共享组件（`components/ui.tsx`）与 `app.css`。
- 非目标：不改 `petsona-core` / `petsona-runtime` 的业务语义；不新增设置项；不改 `config.json` 字段、
  IPC 命令与数据格式；不动浮层（宠物 / 气泡 / Composer）与 macOS；不做 M6 打包、签名与 workflow。
- HEAD / 工作区：HEAD `3ccd819`；未提交为 DI 批次 12 个文件（见执行记录「接手」）。
  执行本计划前应先由用户提交或确认这批改动，避免与 DI 证据混淆。
- 对应执行记录：`../execution/settings-ui-deep-review.md`。

## 审查方法（只读，可复核）

- 构建真实前端产物：`PATH="$HOME/.nvm/versions/node/v24.19.0/bin:$PATH" pnpm --dir apps/desktop build`
  （退出码 0；CSS 21.20 kB / JS 194.92 kB）。
- 用模拟 Tauri 数据桥（`settings_snapshot` / `pet_preview`）+ 无头 Chromium 渲染六页；
  窗口档位：默认窗（905×570，对应 `tauri.conf` 920×600）、宽窗 1180×820、最小窗 640×520。
- 采集方式：截图 + `--dump-dom` 注入 `getBoundingClientRect`/`getComputedStyle` 实测。
  不启动 Petsona、不移动鼠标、不触碰用户数据目录。
- 截图与脚本：`.scratch/ui-review/`（git 忽略）；临时工程 `/tmp/petsona-ui-review/`。
- 限制：Chromium 与 WebView2 渲染可能有细微差异；最终观感由用户实机人工确认。
- 模拟数据：2 只宠物（boba/rocky）、2 条长期偏好、1 条 pending 候选、已拉取 2 个模型、Key 已配置。

## 审查结论（REV 清单）

### P0 必须先修（客观缺陷，已实测）

| REV | 位置 | 事实/证据 | 影响 |
|---|---|---|---|
| REV-01 | `apps/desktop/src/app.css:368`（`.setting-copy { flex: 1 1 220px }`）+ `:398`/窄窗媒体查询 | 纵向模式下 flex-basis 变成**高度 220px**；实测「固定问候文案」整行 333px、人格「补充说明」整行 346px，标签与控件间约 200px 空洞；`≤680px` 时所有设置行都会纵向堆叠 | 默认窗口观感像“断页”；最小窗 640px 整页被撑成长条，是当前最严重的 UI 缺陷 |
| REV-02 | `apps/desktop/src/app.css:753`（`.range`） | Chromium 给 `input[type=range]` 的 UA 默认 `margin: 2px` 未清除，`width:100%` 后向右溢出；实测右缘 开关 1090 / 文本域 1090 / **滑块 1092** | 用户「开关和滑块右缘没对齐」的直接来源之一 |
| REV-03 | `apps/desktop/src/app.css:207`（`.content`） | 同一窗口下，无滚动条的连接页控件右缘 1105，有滚动条的外观页 1090（差 15px）；大窗口居中布局下仍有 7.5px 抖动 | 切页时整页内容宽度跳动，观感“不稳” |

### P1 设置面与信息架构（需要用户拍板）

| REV | 位置 | 事实 | 建议 |
|---|---|---|---|
| REV-04 | `pages/PersonaPage.tsx:159`「表达」卡 | 整卡只有一个 emoji 开关 + 一句解释；`traits.emoji` 默认 `true`，关闭时仅向提示词追加「不要使用 emoji」（`crates/petsona-core/src/persona.rs:119`），与已下架的「语气」属同类低价值旋钮 | **方案 A（推荐）**：删除「表达」卡与开关，字段与默认值保留、系统提示词可替代；**方案 B**：保留开关但去掉卡片标题/描述并入其他卡 |
| REV-05 | `pages/PersonaPage.tsx:124–153` 页头 | 「复制到… / 导入 / 导出 / 重置为内置 / 自动保存」5 个元素同排，动作层级不分主次 | 收敛为 1 个主操作 + 「…」溢出菜单；保存状态降为页脚轻提示 |
| REV-06 | `pages/SystemPage.tsx:92/99/112/123` | 页头「打开仓库」与卡内「访问」重复；`M6 发布验收`、`HKCU\Software\…`、`Rust 原生浮层 + TypeScript 内容界面` 是内部/开发语言；「本机数据」卡描述写的是进程生命周期 | 仓库入口只留一处；提示改为用户语言；进程语义移出或删除 |
| REV-07 | `pages/ConnectionPage.tsx:172/299` | 卡描述「顺序固定为服务商 → Base URL → API Key → 模型」是评审语言；页脚枚举 `config.json` 字段 | 删/改为用户可用的说明；只保留凭据与隐私语义 |
| REV-08 | `pages/MemoryPage.tsx:226/336` | 每条事实一个**实心红删除**按钮，红色被日常操作用满；卡片级「清空全部」反而同色 | 行内删除降级为 hover 显示的图标/文字按钮，实心红只留破坏性卡片动作 |
| REV-09 | `components/PersonaSourcePanel.tsx:342` | 人格草稿卡使用 `tone="danger"`（红边框），语义是“危险”而非“重要” | 改用强调色；草稿是主流程不是危险操作 |
| REV-10 | `pages/PersonaPage.tsx` 高级提示词 | `<details>` 默认折叠，用户无法一眼判断是否已自定义 | summary 旁增加「默认 / 已自定义」状态徽章 |

### P2 一致性与可达性（可排后）

| REV | 位置 | 事实 | 建议 |
|---|---|---|---|
| REV-11 | 全局 | 7 处 `window.confirm`（ChatApp:108、PetsPage:81、PersonaPage:81、ConnectionPage:132、MemoryPage:90/107/228）与自绘 modal（导入冲突、复制人格）两套体系 | 统一为应用内 `ConfirmDialog`（含 Esc、初始焦点、焦点圈闭） |
| REV-12 | `App.tsx` | `.content` 是常驻滚动容器，切页不重置滚动位置 | 切换页面时回到顶部（或按页记忆） |
| REV-13 | `components/ui.tsx:126` + `app.css` | `NumberField` 组件从未使用；`.number-field / .section-toolbar / .wrap / .danger-text / .pet-avatar / .pet-thumb-large` 无引用 | 死代码清理，承接 DI 批次死样式收尾 |
| REV-14 | `app.css:364` | `.setting-row:first-of-type` 在带标题卡片中不会命中（`.card-heading` 才是首个 div），第一行分隔线属“意外生效” | 改为显式 `.card-heading + .setting-row { border-top: 0 }` 或明确保留 |
| REV-15 | `pages/AppearancePage.tsx:79/91` | `1.00×` 显示两次；7 个刻度按钮是滑块的重复入口，且位于 `aria-hidden` 容器中仍可聚焦 | 去重；若保留刻度则 `tabIndex={-1}` |
| REV-16 | `components/ui.tsx:50` | `SettingRow` 用 `<span>` 作标签，输入控件无可编程标签；modal 无焦点管理 | 补 `htmlFor`/`aria-labelledby`/`aria-describedby` 与焦点圈闭 |
| REV-17 | `app.css:324–331` | 所有卡片 `backdrop-filter: blur(16px)` + 阴影 | 卡片改纯色/半透明无 blur，blur 仅留给 popover/modal/toast（滚动性能） |
| REV-18 | `pages/PetsPage.tsx:107` | 导入弹层点击外部/Esc 不关闭 | 补外点与 Esc 关闭 |
| REV-19 | `pages/MemoryPage.tsx` | 事实列表无上限/搜索；空候选卡占一整卡 | 条数多时折叠或搜索；空候选收成一行提示 |
| REV-20 | `pages/PetsPage.tsx:81` | 删除宠物确认未说明人格/记忆保留 | 补一句「人格与记忆会保留，重新导入同 ID 可继续使用」 |

### 已核对但判定保留（不构成改动）

- 宠物页：导入菜单三项、本地库（单击选择/双击切换/设为当前/导出/删除）、Codex 候选。
- 外观与交互：缩放滑块、空闲问候（开关 + 固定文案）；四个无行为开关维持下架。
- 记忆：启用记忆、保存聊天历史、偏好事实（增删改/来源/归档）、待确认习惯、清空全部、导入导出。
- 连接：服务商 / Base URL / API Key（保存·清除）/ 模型（拉取）四段结构与顺序。
- 系统：关于与诊断、开机自启、数据/日志目录、复制诊断、主题说明。
- 连接页输入框“控件组右对齐”（尾部按钮占位导致三个输入框右缘 1105/970/1035）：与 VS Code 等主流设置页一致，
  默认保持；若要“输入框严格同一竖线”需引入固定动作列，属设计变更，需用户明确要求。

## 目标与需求

| REQ | 可验证要求 | 范围 | 完成标准 |
|---|---|---|---|
| REQ-S1-1 | 消除纵向设置行的 200px 空洞（REV-01） | 本次 | 外观「固定问候文案」、人格「补充说明」与草稿字段行高 ≈ 内容 + 间距（≤140px）；640px 宽无空洞 |
| REQ-S1-2 | 滑块右缘与同卡片文本域/开关右缘一致（REV-02） | 本次 | 实测右缘差 ≤1px |
| REQ-S1-3 | 有无滚动条时卡片右缘稳定（REV-03） | 本次 | 同一窗口切页卡片右缘差 ≤1px |
| REQ-S1-4 | 死代码与选择器收尾（REV-13/14） | 本次 | 无未引用组件/样式；第一行分隔线规则显式 |
| REQ-S1-5 | 切页滚动复位（REV-12） | 本次 | 从长页切短页停在页面顶部 |
| REQ-S2-1 | 「表达」卡按 D1 结论处置（REV-04） | 本次（待拍板） | 按拍板结果删除或降级；设置项清单与文档同步 |
| REQ-S2-2 | 系统页去重去术语（REV-06） | 本次 | 无 M6/HKCU/技术栈文案；仓库入口唯一 |
| REQ-S2-3 | 连接页与页脚文案精简（REV-07） | 本次 | 无“顺序固定为…”与字段枚举 |
| REQ-S2-4 | 记忆行内删除降噪 + 草稿卡改色 + 提示词状态（REV-08/09/10） | 本次 | 红色仅用于清空/删除类破坏动作；草稿卡强调色；提示词有状态徽章 |
| REQ-S3-1 | 统一确认对话框（REV-11） | 本次（待拍板 D2） | 7 处 `window.confirm` 全部替换；Esc/焦点可用 |
| REQ-S3-2 | 页头动作收敛（REV-05） | 本次（待拍板 D3） | 每页 ≤1 主操作 + 溢出菜单 |
| REQ-S3-3 | 表单可达性与对比度（REV-15/16） | 本次 | 标签可编程关联；可聚焦元素不再位于 `aria-hidden`；正文提示对比度 ≥4.5:1 |
| REQ-S4-1 | 长尾交互（REV-17/18/19/20） | 后续/可选 | 弹层外点关闭、事实列表折叠、删除确认补语义、blur 性能 |

## 逐文件变更（草案，拍板后细化）

| 路径 | 新增/修改 | 具体改法 | REQ | 前置条件 |
|---|---|---|---|---|
| `apps/desktop/src/app.css` | 修改 | 纵向行 flex 重置、`.range` margin、`scrollbar-gutter`、分隔线选择器、删死样式、对比度变量、卡片 blur | S1、S3 | 无 |
| `apps/desktop/src/pages/AppearancePage.tsx` | 修改 | 刻度/数值去重（如拍板） | S3 | D4 |
| `apps/desktop/src/pages/PersonaPage.tsx` | 修改 | 「表达」卡处置、页头收敛（如拍板） | S2、S3 | D1、D3 |
| `apps/desktop/src/components/PersonaSourcePanel.tsx` | 修改 | 草稿卡 tone、堆叠行受益于 CSS | S2 | 无 |
| `apps/desktop/src/pages/SystemPage.tsx` | 修改 | 去重、去术语、文案重写 | S2 | 无 |
| `apps/desktop/src/pages/ConnectionPage.tsx` | 修改 | 卡描述与页脚精简 | S2 | 无 |
| `apps/desktop/src/pages/MemoryPage.tsx` | 修改 | 行内删除降级、确认框替换、空态收束 | S2、S3 | D2 |
| `apps/desktop/src/pages/PetsPage.tsx` | 修改 | 弹层外点关闭、删除确认语义、确认框替换 | S3、S4 | D2 |
| `apps/desktop/src/components/ui.tsx` | 修改 | label/aria 补全、删 `NumberField` | S1、S3 | 无 |
| `apps/desktop/src/components/ConfirmDialog.tsx` | 新增 | 统一确认对话框（Esc/焦点） | S3 | D2 |
| `apps/desktop/src/App.tsx` | 修改 | 切页滚动复位 | S1 | 无 |
| `docs/plans/settings-ui-deep-review.md`、`docs/execution/settings-ui-deep-review.md` | 修改 | 同步批次结论与证据 | 全部 | 执行时 |

## 必须保持的约束

- 不新增第三方依赖；不新增/不恢复设置项；不改 config 字段、IPC 命令、协议与数据格式。
- emoji 开关若删除：`traits.emoji` 字段、默认值与「不要使用 emoji」提示词语义必须保留（仅下架 UI）。
- 不改 Rust 浮层与托盘行为；不动 macOS。
- 自动测试不移动鼠标、不注入输入；纯观感与交互项列人工验收。
- Git 只读；每轮交付给出全部未提交改动摘要与提交命令。
- 前端构建必须用 nvm node24：`PATH="$HOME/.nvm/versions/node/v24.19.0/bin:$PATH" pnpm --dir apps/desktop build`。

## 验收与命令

| 测试ID | REQ | 目标/前置 | 命令或步骤 | 预期 |
|---|---|---|---|---|
| T-01 | S1–S3 | 前端构建 | `pnpm --dir apps/desktop build` | tsc + vite 退出码 0 |
| T-02 | S1 | 布局回归 | 复用本轮无头渲染审查法（mock 桥 + `--dump-dom` 几何） | 堆叠行 ≤140px；滑块/文本域右缘差 ≤1px；跨页右缘差 ≤1px |
| T-03 | S1–S3 | Windows 壳 | `scripts/desktop-build-windows.ps1`、`scripts/desktop-settings-smoke.ps1` | 构建与冒烟通过（无鼠标） |
| T-04 | S1–S3 | 工作区门禁 | `cargo fmt/clippy/test`（如未触碰 Rust 可只跑受影响项并注明） | 退出码 0 |
| H-01 | S1 | 人工 | 实机默认窗与最小窗（640）目视 | 无空洞、无右缘错位 |
| H-02 | S2 | 人工 | 文案与设置项核对 | 与拍板结果一致，无内部术语 |
| H-03 | S3 | 人工 | 确认框 Esc/焦点、键盘 Tab、对比度观感 | 通过 |

## 顺序、暂停与完成

- 依赖顺序：D1–D5 拍板 → S1（纯 CSS，先落地）→ S2 → S3 → 视情况 S4。
- 执行者可自行决定：S1 内部实现细节、字号/间距的 ±1px 级微调（不得改变 REQ 目标）。
- 必须暂停交回用户：删除或改变任何设置项（D1）、引入新组件（D2/D3）、改变控件组对齐策略、触碰 config 字段。
- 本次完成条件：REQ-S1…S3 全部实现且 T-01–T-04 通过、H-01–H-03 人工通过；S4 可延后但需在文档中保留状态。
- 全项目完成条件：设置页收口后回到 `pre-release-full-acceptance` RG 人工验收，再进入 M6。

## 决定点（执行前必须拍板）

| ID | 问题 | 推荐 | 影响 |
|---|---|---|---|
| D1 | 人格页「表达」卡与 emoji 开关 | 删除整卡与开关，字段保留 | 少一个设置项；关 emoji 改由系统提示词表达 |
| D2 | 是否统一自绘确认对话框 | 做（新增 `ConfirmDialog.tsx`） | 替换 7 处 `window.confirm`，风格统一 |
| D3 | 页头是否收敛为「主操作 + …」 | 做 | 人格/宠物/记忆页动作更清晰 |
| D4 | 缩放刻度按钮与重复数值 | 删刻度按钮，保留一个数值 | 少 7 个可聚焦重复控件 |
| D5 | 连接页输入框是否严格同一竖线 | 保持控件组右对齐 | 若要严格对齐需固定动作列，属设计变更 |

## 修订记录

| 版本 | 用户确认依据 | 改变的REQ/范围/验收 | 原因 |
|---|---|---|---|
| v0.1 草案 | 2026-10-09 用户：DI 通过；「先不要执行，写成文档」 | 初始版本 | 落盘深度审查结论，等待拍板 |

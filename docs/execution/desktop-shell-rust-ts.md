# desktop-shell-rust-ts：执行与审查记录

## 接手

- 计划路径/版本：`docs/plans/desktop-shell-rust-ts.md` v1.0。
- 用户执行授权 / 日期 / 对话标识：2026-10-08 用户回复“开始”（授权 P0 清场 + M0）；此前同日确认 Rust+TS 方向并明确“与最新目标无关的旧代码和文件可以完全摈弃”。
- HEAD、分支、相关dirty/untracked文件和已有用户改动：`main` @ `6bca241`（**旧世界完整快照**，工作区干净）；本轮直接在 `main` 上执行 P0。
- 关键目标与验收复述：提取产品行为规格 → 删除旧前端/FFI/旧脚本/旧文档 → workspace 收为 core+runtime → 重写根文档与 CI → 门禁通过；随后进入 M0（Tauri 骨架 + 六项浮层对照 + macOS 可行性）。
- 本次实际状态：**P0 已提交（`1e3b5c1`）；M0 自动对照已取得数据（透明/穿透/拖动/焦点/启动计时），托盘菜单与观感待人工，macOS 可行性待 Mac。**

## 要求与文件

| REQ | 改动文件与具体行为 | 实现状态 | 自动验证 | 人工验收 | 剩余工作 |
|---|---|---|---|---|---|
| REQ-D01 | 新增 `docs/plans/desktop-shell-rust-ts.md`、`docs/DESKTOP_VERIFICATION.md`、本执行记录 | 已实现 | 文件存在与内容自查 | 待用户抽查 | — |
| REQ-D02 | 提取：设置六页/即时生效/模型/人格/记忆/问候决策 → 计划「继承的产品决策」；W 矩阵行为 → 新验收清单 A–F；协议语义与手工命令 → 新验收清单附录 | 已实现 | 文档自查 | 待用户抽查 | — |
| REQ-D03 | 删除 `apps/windows`、`apps/macos`、`crates/petsona-ffi`、`contracts/`、`scripts/`、旧 workflows、旧计划/执行/验收文档、旧打包工具 | 已实现 | `git status --short`：124 项删除 | 不适用 | — |
| REQ-D04 | `Cargo.toml` 移除 ffi 成员、重写 panic 注释；`Cargo.lock` 同步（-11 行） | 已实现 | fmt/clippy/test 全绿 | 不适用 | — |
| REQ-D05 | 重写 `AGENTS.md`、`README.md`、`CHANGELOG.md`、`docs/PLATFORM_ARCHITECTURE.md` | 已实现（本批落盘） | 文档自查 | 待用户抽查 | — |
| REQ-D06 | `ci.yml` 重写为 Rust-only（macos + windows 矩阵）；删除 release-macos/windows workflow | 已实现 | YAML 语法校验 | 不适用 | release workflow 在 M6 重建 |

## 命令证据（追加，不覆盖失败历史）

| 证据ID/时间 | REQ与代码版本 | cwd/OS/架构/目标入口 | 完整命令 | 退出码/测试数 | 结果路径及限制 |
|---|---|---|---|---|---|
| E-D01 / 2026-10-08 | REQ-D03 | WSL `/home/cwwwwy/Petsona` | 计划内删除清单批量 `rm`（apps×2、ffi、contracts、scripts、旧 workflows/docs/packaging 工具） | exit 0 | `git status --short` 显示 124 D；保留项未触碰 |
| E-D02 / 2026-10-08 | REQ-D04 | WSL / Linux x64 | `cargo fmt --all -- --check` | exit 0 | — |
| E-D03 / 2026-10-08 | REQ-D04 | WSL / Linux x64 | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0；0 警告 | — |
| E-D04a / 2026-10-08 | REQ-D04 | WSL 沙箱内 | `cargo test --workspace` | **失败保留**：6 项因 `cannot bind 127.0.0.1:0 ... Operation not permitted (os error 1)` | 沙箱禁止绑定本地端口，属环境限制，非代码缺陷 |
| E-D04b / 2026-10-08 | REQ-D04 | WSL 沙箱外（用户批准） | `cargo test --workspace` | exit 0；**114/114**（core 86、runtime 28） | 与 E-D04a 对照证明失败为环境性 |
| E-D05 / 2026-10-08 | REQ-D06 | WSL（用户批准联网） | `npx --yes js-yaml .github/workflows/ci.yml` | exit 0；YAML OK | 一次性工具校验语法 |

## 冲突与变更请求

| CR | 文件事实 | 影响REQ/范围 | 建议 | 用户决定/计划版本 |
|---|---|---|---|---|
| 无 | P0 范围内未发现冲突 | — | — | — |

## 审查及关闭

| REV/优先级 | REQ | 位置/触发/影响 | 必需修复与测试 | 关闭证据/状态 |
|---|---|---|---|---|
| 待审 | — | 删除了 124 个文件、重写 6 份文档，等待用户审查与提交 | 用户抽查计划/验收清单内容完整性 | 未关闭 |

## 交付与接续

- 实际完成范围：P0 全部 REQ（D01–D06）。
- 未完成/失败/SKIP/人工待验：M0 全部（骨架、六项浮层对照、macOS 可行性）；桌面实机验证依赖 Windows 环境。
- 对用户现有数据/行为的影响：无——数据目录、`config.json`、协议、宠物库格式未触碰；已构建的旧版验收包仍可运行（源码已删，快照在 `6bca241`）。
- 下一个执行者的安全接续点：读本文件与计划；M0 第一步为环境确认（Windows 侧 Node/pnpm、Rust MSVC、WebView2），然后创建 `apps/desktop` 骨架。
- 旧世界参照方法（只读）：`git show 6bca241:apps/windows/...`、`git show 6bca241:docs/WINDOWS_VERIFICATION.md` 等；不恢复死代码到工作树。
- Git操作是否发生（默认无）：无。用户提交命令见本轮交付说明。
- 完成判定及对应证据：P0 自动门禁通过（E-D02–E-D04b）；M0 未开始，整体迁移未完成。


## M0 执行记录（2026-10-08 接续）

### 范围与结果

| REQ | 结果 | 证据 |
|---|---|---|
| REQ-S01 骨架 | `apps/desktop`（Tauri 2.12 + React 18 + Vite 6 + TS）构建成功，空设置窗可开合 | E-M0-02/03 |
| REQ-S02 原生宠物窗 | `WS_EX_LAYERED` + `UpdateLayeredWindow` 渲染夹具精灵；透明背景截图目检通过 | E-M0-05/09 |
| REQ-S03 穿透与光标 | `WS_EX_TRANSPARENT` 在透明/不透明像素间正确切换；`WM_NCHITTEST` 返回 `HTTRANSPARENT`；类光标为箭头 | E-M0-06 |
| REQ-S04 焦点 | 宠物窗始终不为前台；`--show-settings` 时设置窗取得前台 | E-M0-07 |
| REQ-S05 托盘 | **待人工**：任务栏/收纳面板右键菜单、设置项打开设置窗 | 待人工 |
| REQ-S06 启动耗时 | 3 次实测 485 / 356 / 346 ms（旧基线：热 ~550ms / 冷 ~3200ms），不劣化 | E-M0-04 |
| REQ-S07 macOS 可行性 | **未开始**（需 Mac 环境） | — |
| REQ-S08 决策结论 | 六项中五项已获证据且无阻断问题；待托盘人工确认后关闭 M0 | 本文 |

### 命令证据（追加）

| 证据ID/时间 | REQ | 环境 | 命令/操作 | 结果 |
|---|---|---|---|---|
| E-M0-01 | 环境 | Windows | `node --version` / `pnpm --version` / WebView2 注册表 / rustup toolchain list / vswhere | node v24.19.0、pnpm 12.3.4、WebView2 154.0.4258.62、Rust 1.98.0-msvc（`.rustup` 直调）+ VS BuildTools 18 |
| E-M0-02 | S01 | WSL | `cd apps/desktop && pnpm install && pnpm build` | exit 0；tsc + vite 构建 27 模块；`pnpm-workspace.yaml allowBuilds: esbuild`（pnpm 11 新设置） |
| E-M0-03 | S01 | Windows | `scripts/desktop-build-windows.ps1`（vcvars 导入 + 工具链直调 + 本地 `CARGO_TARGET_DIR`） | exit 0；产物 `C:\Users\happyddz\petsona-build\desktop-target\debug\petsona-desktop.exe` |
| E-M0-04 | S06 | Windows | `desktop-m0-check.ps1` 启动计时 ×3 | 485 / 356 / 346 ms（Start-Process → 宠物窗可见） |
| E-M0-05 | S02 | Windows | 同上，读取窗口 ex-style | LAYERED / TOOLWINDOW / TOPMOST / NOACTIVATE 全部 True |
| E-M0-06 | S03 | Windows | 同上，网格采样 + 样式读取 | transparent=True 与 opaque=True 均观察到（切换生效） |
| E-M0-07 | S04 | Windows | 同上 + `--show-settings` | 宠物前台=False；设置窗可见且前台=True |
| E-M0-08 | S02/S03 | Windows | 合成拖动与真实拖动 | 合成输入受**真实鼠标活动干扰**多次未命中（记录了失败）；随后真实鼠标拖动成功：日志显示窗口 (420,260)→(1246,415) 平滑跟随、drag started/ended 完整 |
| E-M0-09 | S02 | Windows | `scripts/desktop-shot.ps1` 截图 `%TEMP%\petsona-m0-shot.png` | 透明背景、无白底、无边框；桌面内容透过窗口可见 |

### M0 修复的缺陷

| # | 现象 | 根因 | 修复 |
|---|---|---|---|
| 1 | 应用启动即崩溃（exit 0xC0000409） | `SetWindowLongPtrW` 修改样式时同步投递 `WM_STYLECHANGED` 重入窗口过程，`RefCell` 二次借用 panic；panic 跨 `extern "system"` 直接终止进程 | 窗口过程改用 `try_borrow_mut`，重入消息回退 `DefWindowProcW`（`overlay.rs`） |
| 2 | 检查脚本始终找不到宠物窗 | PowerShell 将 `$null` 字符串参数编组为**空串**，`FindWindowW` 变成“查找标题为空的窗口” | 脚本改用 `[NullString]::Value` 传真实 NULL；加注释防止复发 |
| 3 | 合成鼠标测试结果不稳定 | 真实鼠标活动与 `SetCursorPos/mouse_event` 冲突（用户在场操作） | 记录区分（真实拖动为 1px 级高频轨迹）；后续自动化测试需在鼠标空闲时执行 |

### 待人工/后续

- 托盘右键菜单（任务栏可见 + 收纳面板两种状态）、菜单打开设置窗；设置窗打字与中文 IME。
- 真实穿透点击（透明处落桌面、宠物像素可点）与拖动手感目视。
- macOS objc2 可行性（需 Mac）。


## M1 执行记录（2026-10-08 接续）

### 范围细化（「M1 骨架」的逐项 REQ）

| REQ | 内容 | 结果 |
|---|---|---|
| M1-01 引擎接入 | `apps/desktop/src-tauri` 直接依赖 `petsona-runtime`（无 FFI）：worker 线程负责数据目录（`PETSONA_HOME`/默认）、日志（`logs/petsona.log`）、`petsona.lock` 单实例、配置与宠物加载 | 完成 |
| M1-02 状态协议 | 由 runtime 绑定 `stateServer.port`（默认 17872）；`/health`、`/pets`、`/state`（TTL、非法状态 400、`action:clear`）与运行时状态联动 | 完成（隔离 home 用 17897/17898 验证） |
| M1-03 托盘菜单 | 设置… / 显示-隐藏宠物 / 退出（Tauri tray-icon）。**细化说明**：宠物库/缩放/历史项在 M2/M3 拥有实际目标后加入，避免死菜单项（执行记录已标注，非静默缩减） | 完成（人工点验待补） |
| M1-04 空库首启 | `ready && !has_pet` → 自动打开设置窗（一次性） | 完成 |
| M1-05 浮层可见性 | 由 runtime 驱动：`ready && has_pet && pet_visible` 才显示；托盘显隐即时生效 | 完成 |
| M1-06 退出与端口 | 托盘退出 / `--exit-after-ms` 走 `engine.stop()` + `app.exit(0)`；端口释放 | 完成 |

### 命令证据（追加）

| 证据ID/时间 | REQ | 环境 | 操作 | 结果 |
|---|---|---|---|---|
| E-M1-01 | M1-01 | WSL→Windows | `scripts/desktop-build-windows.ps1`（新增 petsona-runtime 路径依赖；workspace 继承正常） | exit 0 |
| E-M1-02 | M1-01..06 | Windows | **`scripts/desktop-smoke.ps1` 完整运行（EXIT=0）**；隔离 homes `%TEMP%\petsona-smoke-pet`（v2 夹具宠物 + activePet）与 `petsona-smoke-empty`（无宠物），端口 17897/17898 | 全部分节通过 |
| E-M1-02a | 启动 | 同上 | 启动计时 ×3（runtime 驱动，宠物可见） | 751 / 793 / 759 ms |
| E-M1-02b | 样式/穿透/拖动/焦点 | 同上 | 样式 4 项全 True；穿透 transparent/opaque 均切换；拖动 attempt 2 成功 dx=100 dy=60（attempt 1 受真实鼠标干扰失败）；宠物前台=False | 通过 |
| E-M1-02c | 单实例 | 同上 | 第二进程退出、存活 1 个、第一个实例 /health 正常 | 通过 |
| E-M1-02d | 协议 | 同上 | /health（pet='TestPet'、pets=[test_fixture_v2]）、/pets 包含夹具、POST waiting→TTL→idle、非法状态 HTTP 400、action:clear 接受 | 通过 |
| E-M1-02e | 退出 | 同上 | `--exit-after-ms 3000`：进程退出、17897 不再监听 | 通过 |
| E-M1-02f | 空库首启 | 同上 | 设置窗自动可见、宠物窗隐藏、/health `pet` 为空 | 通过 |

### 备注与偏差

- 启动 0.75–0.88s（含 runtime 配置/宠物加载与真实呈现路径），高于旧 C# 热启动基线 ~0.55s；不阻塞 M1，列入 M6 性能收尾评估。
- 第二实例当前**静默退出**（写日志）；旧壳的「故障/锁冲突提示气泡 + 3 秒退出」待 M2 气泡窗口就绪后补齐。
- 合成拖动测试加固：最多 3 次重试 + 取离窗口中心最近的可点击像素；真实鼠标活动仍可能造成单次假失败（日志可辨）。
- 新配置事实：隔离 home 的 `config.json` 端口键必须是 **`stateServer`**（camelCase）；写成 `state` 会被 serde 默认值忽略并回落 17872（M1 首跑即踩到，已在脚本中修正）。
- 脚本演化：`desktop-m0-check.ps1` 更名为 `desktop-smoke.ps1`（保留 M0 检查并新增 M1 六节；`-SkipMouseChecks` 供鼠标忙时使用）。


## M2-A 执行记录（2026-10-08 接续）

范围：真实图集渲染 + 动画时序 + 缩放/DPI + 位置持久化与工作区夹取（M2 的前半；注视/气泡/编辑条/Composer 留 M2-B）。

### REQ

| REQ | 内容 | 结果 |
|---|---|---|
| M2-A-01 渲染器 | 浮层按 runtime 快照渲染：`atlas_path` + `sprite_index` + `cell(192×208)` × `scale` × DPI（钳制 ≥1）；帧缓存（上限 256）；命中掩码 = 当前帧 ∪ idle 行并集 | 完成 |
| M2-A-02 动画时序 | 33ms 可视化轮询，仅在 sprite/尺寸变化时重绘；帧由 runtime 引擎推进（不再使用内嵌夹具固定 120ms 动画） | 完成 |
| M2-A-03 缩放/锚点 | 尺寸变化按 bottom-centre 锚点移动并夹取到工作区；DPI 变化走同一路径 | 完成（DPI 变化实机待多显示器场景） |
| M2-A-04 位置记忆 | 拖动结束发送 `SetPosition`（写入 `config.window.startPosition`，物理像素）；启动先应用记忆位置/默认右下再显示；全程 `rcWork` 夹取（任务栏为边界） | 完成 |

### 命令证据（追加）

| 证据ID/时间 | REQ | 环境 | 操作 | 结果 |
|---|---|---|---|---|
| E-M2-A-01 | M2-A-01 | WSL→Windows | 构建（image 增加 webp/jpeg 特性；windows-sys 增加 HiDpi） | exit 0 |
| E-M2-A-02 | M2-A-01/03/04 | Windows 真实数据目录（activePet=boba, scale 0.5） | 启动 + 截图 + 读窗口矩形 + /health | boba 真实图集渲染成功（截图 `%TEMP%\petsona-boba-shot.png`）；rect=1338,487 size=96×104（=192×208×0.5，且等于记忆位置）；/health pet=Boba |
| E-M2-A-03 | M2-A-01..04 | Windows 隔离 home（夹具, scale 1.5） | `desktop-smoke.ps1` 完整运行 | **EXIT=0**：几何 expected=actual=288×312；动画 14 采样出现 2 种帧；穿透切换 ✓；拖动 MOVED（24px，真实鼠标干扰下取部分移动）→ 新保存 1632,720 → 重启恢复 dx=0/dy=0；单实例/协议/退出/空库回归全过 |
| E-M2-A-04 | 启动测量 | Windows | 同上 + 真实目录 | 夹具 home ~1.74–1.78s（进程内首帧 +1.13s）；boba 真实 home 本次 ~3.0s（webp 解码更重、波动大）——列入 M6 性能收尾 |

### 备注与发现

- **V2 渲染格是 canonical 192×208**：runtime 不读取宠物包里的 `grid.cellWidth/Height`（测试夹具声明 64×64，实际仍按 192×208 渲染）。冒烟几何预期已按此修正；旧 C# 前端行为一致。
- 拖动冒烟在活跃真实鼠标环境下的策略：取离窗口中心最近的可点击像素 + 重试 + “部分移动即通过（|dx|+|dy|≥8）”；位置往返（保存→重启恢复）已可独立证明。
- 启动耗时较 M1（~0.75s）上升（夹具 ~1.7s、boba ~3.0s）；in-app 首帧约 +1.1s。原因待 M6 性能收尾时用固定方法（多次热启动 + 日志分段）排查；不阻塞功能验收。
- 故障/锁冲突提示气泡仍待 M2-B（当前静默退出 + 日志）。


## 测试策略变更与 M2-B-01 注视（2026-10-08 接续）

### 测试策略变更（用户要求）

- 用户明确：自动测试**不得添加需要移动鼠标的项目**，这类验收改由用户手动执行。
- `desktop-smoke.ps1` 去鼠标化：移除 `SetCursorPos`/`mouse_event`、穿透采样与合成拖动段；新增**无鼠标**的位置恢复检查
  （写入已知 `startPosition=(200,200)`，启动后断言窗口位置）；脚本末尾打印人工清单（穿透、拖动夹取、位置记忆、光标、注视、托盘）。
- 历史记录中的合成鼠标测试（M0–M2-A）保留原样作为历史证据，不再作为默认门禁运行。

### M2-B-01 注视（实机验收待用户）

- 移植自 `GazeFilter.cs` / `GazeStabilizer.cs`（git `6bca241`，含用户此前要求的范围扩大与近距离稳定）：
  - 进入留白 = 短边 80%，退出留白 = 短边 100%（迟滞椭圆）；中心死区 = 短边 35%（近距离保持中性，避免方向翻转）；
  - 22.5° 16 方向量化 + 7° 角迟滞 + 2px 最小移动；命中后**每 16ms 重发稳定方向的单位向量**（光标静止也能完成跨行过渡）。
- 采样并入 16ms HIT 轮询；宠物隐藏或拖动中 `ClearGaze`；离开触发区或进入死区时清空并重置迟滞。

### 证据

| 证据ID/时间 | REQ | 操作 | 结果 |
|---|---|---|---|
| E-M2-B-01 | 策略 + 注视 | 构建（exit 0）+ `desktop-smoke.ps1`（无鼠标）完整运行 | **EXIT=0**：位置恢复 200,200=200,200；几何 288×312；动画 2 帧；样式/焦点/设置聚焦/单实例/协议/退出/空库全过 |

### 待人工

- 注视：全方向（含宠物上方）、近距离保持中性、边界不抖动；
- 既有鼠标项：穿透点击、拖动与边缘夹取、拖动后重启位置记忆、光标形状、托盘三项。


## M2-B-02 气泡（2026-10-08 接续）

- 新模块 `apps/desktop/src-tauri/src/gdi_text.rs`：GDI+ 扁平 API 文字测量/绘制（Microsoft YaHei UI 14pt、AntiAliasGridFit、
  Segoe UI 回退），与旧 System.Drawing 渲染一致；写入预乘 BGRA 缓冲。
- 新窗口 `PetsonaOverlayWindow`（LAYERED/TOOLWINDOW/NOACTIVATE/TOPMOST）：圆角 12、1px 边框；四色取自 `BubblePalette.cs`
  （浅/深色随系统主题，经 Tauri `ThemeChanged` 同步）；进度条 3px；布局常量与旧版一致（padding 16×12、最大文本宽 300、宽域 120–332）。
- 时序：generation 变化 → 150ms 淡入（`SourceConstantAlpha` 渐变）；进度 = remaining/total（runtime 计算）；文案或进度 1/120 变化才重绘。
- 悬停暂停：光标轮询（不注入输入）判定悬停 → `SetBubblePaused(true)`（按 generation 记录）；移开 → `SetBubblePaused(false)` 从剩余时间续跑。
- 定位：宠物上方居中；上方空间不足翻到下方；`rcWork` 夹取（`OverlayLayout.PositionBubble` 移植，EdgeMargin=8、间距=10）。
- 事实记录：协议气泡生命周期为 **固定 8 秒**（runtime 既有行为，与 `ttlMs` 无关；旧版一致）。

### 证据

| 证据ID/时间 | 操作 | 结果 |
|---|---|---|
| E-M2-B-02a | 真实数据目录 + 协议 POST 中文 message，截图 `%TEMP%\petsona-bubble-shot.png` | 圆角面板/中文抗锯齿文字/蓝色进度条/位于宠物上方，全部正确 |
| E-M2-B-02b | 无鼠标冒烟 `desktop-smoke.ps1`（新增气泡可见性检查） | **EXIT=0**：bubble visible after POST=True；hidden after ~8s=True；启动/位置恢复/几何/动画/焦点/单实例/协议/退出/空库全部通过 |

### 待人工（用户）

- 淡入观感；悬停暂停与移开续跑；宠物贴近屏幕顶部时气泡翻到下方；连续新消息重新淡入；点击气泡当前为 no-op（Composer 在 M2-B-04 接入）。


## M2-B-03 Composer 输入框（2026-10-08 接续）

- 实现：可激活的无边框弹出窗 `PetsonaComposerWindow`（360×56、圆角 region 12、WS_EX_TOOLWINDOW 不含任务栏/Alt-Tab），
  内含原生 **EDIT 子控件**（多行 + `ES_WANTRETURN`，IME 原生支持）与自绘圆形发送键（`↑`，GDI Ellipse + DrawText）；
  底色/文字随主题（`WM_CTLCOLOREDIT` + `WM_PAINT`，主题经 Tauri `ThemeChanged` 同步）。
- 行为（对齐旧 ComposerWindow）：
  - Enter 发送（`RuntimeCommand::SendConversation`）并清空输入；**Shift+Enter 换行**（`ES_WANTRETURN` 默认行为）；
  - Esc 关闭并**保留草稿**；再次打开恢复草稿；
  - 打开即尝试前台聚焦（`SetForegroundWindow`+`SetFocus`，1.5s 重试）；
  - 跟随宠物：每帧与拖动中重定位（位置未变时不重复 `SetWindowPos`）；下方空间不足时移到左/右余量更大一侧，之后 `rcWork` 夹取；
  - 入口：**点击气泡**打开（编辑条 M2-B-04 接入后同入口）；另有开发参数 `--open-composer` 与测试消息 `WM_APP+1`。
- 冒烟为**仅窗口消息**（不移动鼠标）：`AllowSetForegroundWindow` 授予前台 → `WM_APP+1` 打开 → `WM_CHAR` 输入 → `WM_GETTEXT` 读回
  → `WM_KEYDOWN` Esc/Return 驱动关闭/发送；清空断言以应用日志为准（跨进程 `WM_GETTEXT` 回读可能滞后）。

### 证据

| 证据ID/时间 | 操作 | 结果 |
|---|---|---|
| E-M2-B-03a | `--open-composer` 启动 + 截图 `%TEMP%\petsona-composer-shot.png` | 白色圆角卡片 + 蓝色 `↑` 发送键渲染正确 |
| E-M2-B-03b | 无鼠标冒烟 `desktop-smoke.ps1`（新增 composer 段） | **EXIT=0**：composer visible=True；foreground=composer=True；edit found=True；typed='hi'；Esc 隐藏=True；reopen 草稿='hi'；Enter 发送并清空（app log `cleared result=1 remaining=''`）；其余全部回归通过 |

### 待人工

- 中文 IME 组合不误发；Shift+Enter 换行；真实点击气泡打开并可直接输入；
- 拖动宠物时输入框跟随、贴近任务栏时左右侧挂；发送后运行时反应（无 Key 时记录错误属预期）。

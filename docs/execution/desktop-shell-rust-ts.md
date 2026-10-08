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

# windows-overlay-interaction：执行与审查记录

## 接手

- 计划路径/版本：`docs/plans/windows-overlay-interaction.md` v1.0。
- 用户执行授权 / 日期：2026-09-22，“其他的我都同意你的，开始执行吧”。
- HEAD、分支、dirty/untracked：`main` @ `f174261`；开始时空工作区，本轮未创建分支。
- 关键目标与验收复述：宠物工作区边界；气泡淡入/进度/悬停暂停；矩形条悬停展开并左右侧挂；无边框 Composer 跟随宠物。
- 本次实际状态：**实施完成；自动门禁已通过；W29–W32 实机观感待用户验收**。

## 要求与文件

| REQ | 改动文件与具体行为 | 实现状态 | 自动验证 | 人工验收 | 剩余工作 |
|---|---|---|---|---|---|
| OV01 | `WindowGeometry.cs`、`OverlayLayout.cs`、`NativeWin32.cs`、`PetWindow.cs`：以当前显示器 `rcWork` 夹取拖动/恢复/尺寸变化后的宠物窗口 | 已实现 | `OverlayLayoutTests` + smoke N26 | 待 W29 | 多屏/混合 DPI 仍按 W12 暂缓 |
| OV02 | `session.rs`、`commands.rs`、`engine.rs`、FFI/ABI 文档、`OverlayWindow.cs`、`LayeredPresenter.cs`：runtime 暴露 `BubbleTiming`，支持暂停/继续，前端淡入和进度条 | 已实现 | runtime `bubble_pause_preserves_remaining_time`；C# `BubblePauseSurvivesPastTheOriginalDeadline` | 待 W30 | 实机确认淡入、进度和悬停观感 |
| OV03 | `OverlayWindow.cs`、`AppController.cs`、`OverlayLayout.cs`：36×6 / 72×6 矩形条，底部优先，左右侧挂，220ms 悬停展开，点击立即打开 | 已实现 | smoke N5/N18/N27 | 待 W31 | 实机确认悬停时序和矩形条观感 |
| OV04 | `ComposerWindow.xaml(.cs)`、`WindowTheme.cs`、`AppController.cs`：隐藏标题栏和边框、移除提示行、紧凑 360×56，拖动时跟随并按底部/左右定位 | 已实现 | smoke N18/N26/N27；诊断截图 `composer-light.png` | 待 W32 | 实机确认无边框、IME、跟随与焦点 |
| OV05 | `windows-smoke.ps1`、`overlay-shots.ps1`、`docs/WINDOWS_VERIFICATION.md`：新增 N26/N27、W29–W32、诊断和验收说明 | 已实现 | `verify-windows.ps1 -Full` exit 0 | 待人工 | 记录用户复测结果 |

## 命令证据（追加，不覆盖失败历史）

| 证据ID/时间 | REQ与代码版本 | cwd/OS/架构/目标入口 | 完整命令 | 退出码/测试数 | 结果路径及限制 |
|---|---|---|---|---|---|
| E-OV01 | 基线 | WSL `/home/cwwwwy/Petsona` | `git status --short`、读取计划与实现 | exit 0 | HEAD `f174261`，开始工作区干净 |
| E-OV02 | OV01–OV05 | WSL / Linux x64 | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` | exit 0；Rust 87 项通过 | core 70 / ffi 4 / runtime 13；无失败 |
| E-OV03 | OV01–OV04 | WSL 调用 Windows .NET 10 | `dotnet test apps/windows/Petsona.sln -c Release -p:Platform=x64 --no-restore -p:WindowsAppSDKSelfContained=false -p:SelfContained=false` | exit 0；44/44 | 新增 6 项 OverlayLayout、1 项气泡暂停和 ABI 枚举断言 |
| E-OV04 | OV01–OV05 | Windows PowerShell 5.1 / UNC 仓库 | `powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1 -Full` | exit 0；native smoke 27/27 ×2 | 覆盖 N26 工作区夹取、N27 贴底侧挂；UNC 下为 framework-dependent 构建 |
| E-OV05 | OV02/OV04 视觉诊断 | Windows PowerShell；隔离 home | `scripts\diagnostics\overlay-shots.ps1 -Theme light -OutputDirectory %TEMP%\petsona-overlay-shots-new3` | exit 0 | `bubble-light.png` / `composer-light.png` 已生成；仅诊断，不等同人工验收 |
| E-OV06 | OV05 失败/修正历史 | Windows PowerShell | 首次 `-Full` 前旧 smoke 的 editButton 查找仍按 40×40 圆钮 | 已修正 | 已改为矩形条尺寸与悬停路径，不保留旧失败结果 |
| E-OV07 | OV01–OV05 最终改动 | WSL + Windows .NET 10 | `cargo fmt/clippy/test`；`dotnet format --verify-no-changes`；`cargo build -p petsona-ffi --release --target x86_64-pc-windows-gnu`；`dotnet build/test`；`scripts\windows-smoke.ps1` | exit 0；Rust 87/87、C# 44/44、smoke 27/27 | 最终 smoke 复跑 27/27；本会话 Windows 侧 `rustc` 已不在 PATH，`verify-windows.ps1 -Full` 的完整脚本复跑由 WSL 等价门禁 + 原生 smoke 覆盖 |
| E-OV08 | OV03/OV05 矩形条定位修正 | Windows PowerShell；隔离 home | 修正竖向展开条的定位判断后重跑 `dotnet build/test` 与 `scripts\windows-smoke.ps1` | exit 0；C# 44/44、smoke 27/27 | N18/N27 均通过；桌面验收包两个目录已镜像当前 Release 产物 |

## 冲突与变更请求

| CR | 文件事实 | 影响 | 建议 | 用户决定/计划版本 |
|---|---|---|---|---|
| 无 | 本轮未发现需要改契约的冲突 | — | — | — |

## 审查及关闭

| REV/优先级 | REQ | 位置/触发/影响 | 必需修复与测试 | 关闭证据/状态 |
|---|---|---|---|---|
| REV-01 | OV04 | `AppController.SampleGaze` 原逻辑在 Composer 打开时每个 tick 清空 caret gaze | 保留 Composer 期间的 caret 目标，关闭时清理 | 代码已调整；跟随/注视仍需 W32 实机确认 |
| REV-02 | OV02 | 气泡计时不能只靠 C# 本地计时，否则 hover pause 与实际 TTL 漂移 | runtime 计算剩余时间，前端只渲染 | E-OV02/E-OV03 通过 |
| REV-03 | OV01 | 自动 smoke 只覆盖当前显示器主屏 `rcWork` | 多屏/混合 DPI/自动隐藏任务栏保留待 W12 | 未关闭，明确标注暂缓 |

## 交付与接续

- 实际完成范围：工作区边界、runtime 气泡计时/暂停、气泡淡入与进度条、编辑条侧挂与悬停、紧凑 Composer、以下自动门禁与文档。
- 未完成/失败/SKIP/人工待验：W29–W32 全部待用户实机复测；W12 的多屏/混合 DPI/自动隐藏任务栏仍暂缓。
- 对用户现有数据/行为的影响：未改 `PETSONA_ABI_VERSION=3` 和结构体布局；新增文本字段 18、命令 43；位置配置仍为物理像素；Composer 尺寸和入口形态有可见变化。
- 下一个执行者的安全接续点：先读取本文件、W29–W32 实机反馈，再用 `scripts\verify-windows.ps1 -Full` 复现基线。
- 本轮验收包：`C:\Users\happyddz\Desktop\Petsona-验收-修复版\启动隔离验收.cmd`；`Petsona-windows-x64-0.1.0` 与 `-polish` 已刷新到当前 Release 产物（隔离 home `%TEMP%\petsona-acceptance2`，端口 17873）。
- Git操作是否发生：无。
- 完成判定：自动通过，人工未完成，**不能宣布整体验收通过**。

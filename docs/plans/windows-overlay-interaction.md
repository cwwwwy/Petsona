# windows-overlay-interaction：执行契约

## 1. 身份与授权

- 任务 ID：`windows-overlay-interaction`；版本：`1.0`；日期：`2026-09-22`。
- 用户目标与已确认决定（2026-09-22 对话）：
  1. 宠物不得移出显示器工作区；Windows 任务栏属于边界。
  2. 气泡增加淡入、剩余时间进度条；鼠标悬停暂停，移开后从剩余时间继续。
  3. 删除影子→编辑按钮过渡；编辑入口改为小矩形条，悬停约 220ms 后展开并聚焦输入框，点击可立即打开。
  4. 输入框无边框、无标题栏、无快捷键提示，只保留输入区和发送按钮，并始终跟随宠物。
  5. 输入区优先在宠物下方；下方空间不足时自动侧挂到左/右空间更大的一侧。矩形条随方向旋转，输入框宽度 `360 -> 最小 280` 自适应，输入期间锁定侧向。
  6. 气泡上方空间不足时翻到宠物下方。
- 本次范围：Windows 原生前端 + 气泡计时所需的最小共享 runtime/FFI 扩展、自动测试、Windows 验收文档和诊断脚本。
- 非目标：macOS 前端实现同构侧挂/悬停动画；发布打包；自动隐藏任务栏的专门适配（记录为人工风险）；重写宠物动画/注视。
- 基线：`main` @ `f174261`，工作区干净（2026-09-22 核对）。
- 对应执行记录：`../execution/windows-overlay-interaction.md`。

## 2. 目标与需求

| REQ | 可验证要求 | 范围 | 完成标准 |
|---|---|---|---|
| REQ-OV01 | 宠物拖动、恢复位置、帧尺寸/缩放变化后，窗口完整位于当前显示器 `rcWork` 内；任务栏在任意边时均作为边界 | Windows | 纯几何测试覆盖负坐标/多屏工作区；smoke 拖动到四边后窗口矩形不越界 |
| REQ-OV02 | 气泡 150ms 淡入，底部 2px 剩余时间进度条；悬停暂停，离开续跑；新气泡重置动画和计时 | 共享 + Windows | runtime 测试暂停/继续/过期；C# EngineClient 端到端测试；人工确认观感 |
| REQ-OV03 | 编辑入口为小圆角矩形条；下方空间不足自动移至左/右；悬停 220ms 展开输入框，点击立即打开 | Windows | 自动 smoke 能识别矩形条/Composer 位置；人工确认悬停时序 |
| REQ-OV04 | Composer 无标题栏/边框/提示行，只显示输入区和发送按钮；始终跟随宠物并按底部优先、左右回退定位 | Windows | 自动检查窗口尺寸/位置和跟随；人工确认样式、Enter/Shift+Enter/Esc/IME |
| REQ-OV05 | 本轮行为、已知限制和人工验收项写入文档；失败与 SKIP 保留 | 文档/脚本 | `docs/WINDOWS_VERIFICATION.md` 和诊断脚本更新；门禁通过 |

## 3. 逐文件变更

| 路径 | 变更 | 具体行为 | REQ |
|---|---|---|---|
| `crates/petsona-runtime/src/session.rs` | 修改 | 气泡保存 `total`、`until`、暂停剩余时间、generation；提供剩余时间/暂停方法 | OV02 |
| `crates/petsona-runtime/src/commands.rs` | 修改 | 新增暂停/继续气泡命令 | OV02 |
| `crates/petsona-runtime/src/engine.rs` | 修改 | 处理命令、暂停时不因 TTL 清理、发布 `BubbleTiming` | OV02 |
| `crates/petsona-runtime/src/snapshot.rs` | 修改 | 新增 `BubbleTiming` 文本字段 | OV02 |
| `crates/petsona-ffi/src/types.rs`、`commands.rs`、`render.rs` | 修改 | 转发字段/命令，保持 ABI 3 结构布局 | OV02 |
| `contracts/petsona.h`、`contracts/ABI.md` | 修改 | 记录 additive 字段 18、命令 43 | OV02 |
| `apps/windows/Petsona.Core/Interop/NativeMethods.cs` | 修改 | 镜像新枚举；保持 ABI 3 布局 | OV02 |
| `apps/windows/Petsona.Core/OverlayLayout.cs`、`WindowGeometry.cs` | 新增 | 纯函数工作区夹取、底部/左右侧选择和定位 | OV01/OV03/OV04 |
| `apps/windows/Petsona/Native/NativeWin32.cs` | 修改 | 增加 per-monitor work area、鼠标离开跟踪、窗口样式辅助 | OV01/OV02/OV03 |
| `apps/windows/Petsona/Native/PetWindow.cs` | 修改 | 拖动/恢复/改尺寸统一夹取工作区，移动时通知浮层 | OV01 |
| `apps/windows/Petsona/Native/OverlayWindow.cs` | 重写/修改 | 气泡淡入、进度条、悬停事件；编辑条横/竖方向与悬停动画 | OV02/OV03 |
| `apps/windows/Petsona/Native/LayeredPresenter.cs` | 修改 | 支持淡入 alpha | OV02 |
| `apps/windows/Petsona/Views/ComposerWindow.xaml(.cs)` | 修改 | 无标题栏/边框，紧凑输入区+发送按钮，固定尺寸与跟随接口 | OV04 |
| `apps/windows/Petsona/AppController.cs` | 修改 | 编排悬停、共享气泡计时、底部/侧挂布局、Composer 跟随 | OV01-OV04 |
| `apps/windows/Petsona.Tests/*` | 修改 | ABI/命令、气泡暂停、纯布局测试 | OV01-OV04 |
| `scripts/windows-smoke.ps1`、`scripts/diagnostics/*` | 修改 | 增加边界、条/Composer 侧挂、气泡计时与截图诊断 | OV01-OV05 |
| `docs/WINDOWS_VERIFICATION.md` | 修改 | 新增人工验收步骤 | OV-05 |

## 4. 必须保持的约束

- Git 只读；不执行 add/commit/push/tag。
- 不新增第三方依赖。
- 所有测试隔离 `PETSONA_HOME`、TCP 端口、自启项。
- 气泡/编辑条保持 `WS_EX_NOACTIVATE`；Composer 可激活并保持 IME/快捷键语义。
- 透明浮层不得重新引入系统边框、忙碌光标或焦点抢占。
- `PETSONA_ABI_VERSION` 保持 3；仅追加枚举值，不改变任何结构体布局。
- 多显示器以光标/窗口所在显示器 `rcWork` 为准；物理像素仍是位置单位。
- 自动隐藏任务栏和 macOS 观感不在本机自动验收范围内，必须标为待人工/待 Mac。

## 5. 验收与命令

| 测试ID | REQ | 目标 | 命令/步骤 | 预期 |
|---|---|---|---|---|
| T-01 | OV01/OV02/OV03/OV04 | Rust workspace | `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` | exit 0；新 runtime 测试通过 |
| T-02 | OV01-OV04 | Windows C# | `dotnet test apps/windows/Petsona.sln -c Release -p:Platform=x64` | 新增 ABI、计时、布局测试通过 |
| T-03 | OV01-OV05 | Windows 门禁 | `powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1` | exit 0 |
| T-04 | OV01-OV05 | Windows smoke | `powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1 -Full` | 新增 smoke 通过；不移动鼠标运行 |
| M-01 | OV01 | 实机 | 拖到四边/任务栏、重启恢复 | 宠物不越出工作区 |
| M-02 | OV02 | 实机 | 触发气泡、悬停/移开 | 淡入、进度暂停/续跑、实际消失一致 |
| M-03 | OV03 | 实机 | 宠物置底、悬停/点击矩形条 | 自动侧挂，220ms 展开并聚焦 |
| M-04 | OV04 | 实机 | 输入中文、Enter/Shift+Enter/Esc、拖动宠物 | 无边框、无提示行、输入框跟随，快捷键正常 |

## 6. 顺序、暂停与完成

- 顺序：共享气泡计时/FFI -> 纯几何/C# ABI -> 宠物边界 -> 浮层与 Composer -> smoke/文档 -> 全门禁。
- 局部选择：具体颜色、圆角、动画缓动由执行者按现有 Theme token 选择。
- 暂停条件：必须破坏 ABI 结构、需要新增第三方依赖、无法同时保持 IME 与无边框、自动隐藏任务栏无法验证。
- 完成条件：T-01～T-04 通过；M-01～M-04 明确列为待用户实机验收，不提前宣布人工通过。

## 7. 修订记录

| 版本 | 用户确认依据 | 改变的内容 | 原因 |
|---|---|---|---|
| 1.0 | 2026-09-22 “其他的我都同意你的，开始执行吧” | 初始合同 | 用户授权执行 |

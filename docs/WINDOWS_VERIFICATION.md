# Petsona Windows 实机验证清单

这份清单用于在 Windows 上人工验收 Petsona。`scripts\verify-windows.ps1` 只证明代码可以
通过格式、Clippy、测试和 release 链接，不能证明窗口、托盘、穿透和系统集成真的可用；
真正的结论以这份清单为准。

当前已知状态（2026-09-14）：

- ✅ `scripts\verify-windows.ps1` 已通过：`cargo fmt`、`cargo clippy`、工作区测试和
  release 链接均成功。
- ✅ `scripts\verify-windows.ps1 -Full` 已接入 `scripts\windows-smoke.ps1` 和默认关闭的
  `test-hooks`，并自动通过 15 项：T1 控制通道、A1、A4（设置动作，部分）、A8（协议/TTL）、
  A9、A11（隐藏/显示，部分）、A13（加速自动行走，部分）、B7（Win32 激活守卫）、
  B8（窗口样式与缩放稳定）、B9（气泡几何，部分）、B11（事件驱动鼠标唤醒）、
  B12–B14、C6（宠物窗口与菜单样式）。
- ⚠️ Windows 实机回归已由用户按表更新；未标注“自动化通过”的项目仍以实机结果为准。
- ⚠️ 多屏、位置记忆和 Windows 发布包仍属于后续 Phase 2/4，相关条目标为“待实现”，
  不计入当前基线失败。
- 当前分支暂不提交 PR、不合并 `main`，继续在 `codex/cross` 上开发；合并时机由用户决定。

最小可用判定：**A1–A3、A8–A11、B1–B11 全部通过**。有副屏条件时，还必须通过 C1–C3；
没有副屏时对应条目标记为“条件不足”，不要用 CI 结果代替。

## 0. 准备

在仓库根目录执行自动门禁：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1
```

也可以双击 `scripts\verify-windows.cmd`。脚本失败时先解决 Rust 门禁，再做本清单。
MSVC 工具链缺少 `link.exe` 时，可在当前 PowerShell 会话使用 GNU 回退：

```powershell
$env:RUSTUP_TOOLCHAIN = "stable-x86_64-pc-windows-gnu"
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = "$env:USERPROFILE\.rustup\toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\rust-lld.exe"
$env:RUSTFLAGS = "-C link-self-contained=yes"
powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1
```

建议使用独立数据目录，避免污染真实配置：

```powershell
$env:PETSONA_HOME = Join-Path $env:TEMP "petsona-win-test"
cargo run -p petsona-app
```

- 默认数据目录：`%APPDATA%\Petsona`。
- 覆盖数据目录：`PETSONA_HOME` 指定的目录。
- 日志：`<数据目录>\logs\petsona.log`，同时输出到终端。
- 单实例锁：`<数据目录>\petsona.lock`。
- release 产物：`target\release\petsona.exe`。
- 查看日志：

```powershell
Get-Content -LiteralPath "$env:APPDATA\Petsona\logs\petsona.log" -Wait
```

状态协议命令（默认端口；如果设置了 `PETSONA_HOME`，行为不变）：

```powershell
Invoke-RestMethod http://127.0.0.1:17872/health
Invoke-RestMethod http://127.0.0.1:17872/pets
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:17872/state `
  -ContentType 'application/json' `
  -Body '{"source":"win-verify","state":"waiting","message":"Windows 验证","ttlMs":10000}'
```

## A. 基础回归

| # | 操作 | 预期结果 | 当前状态 |
|---|---|---|---|
| A1 | 启动 `cargo run -p petsona-app` | 宠物窗口出现；透明背景不是黑底/白底；无标题栏和边框；置顶；任务栏不出现额外宠物窗口 | 通过 |
| A2 | 看系统托盘 | `Petsona` 图标出现；图标使用当前宠物首帧；鼠标悬停显示 `Petsona` | 通过 |
| A3 | 单击/右键托盘图标 | 在光标附近弹出 Petsona 自绘菜单，而不是阻塞事件循环的原生 `TrackPopupMenu`；菜单四项为打开设置、更换宠物、隐藏/显示宠物、退出 | 通过 |
| A4 | 点击菜单各项 | 打开设置、切换到宠物页、隐藏/显示宠物、退出分别生效；菜单点击后消失，不留下透明小窗 | 通过；设置开/关已自动化 |
| A5 | 打开设置 | 设置窗口带原生边框，尺寸和滚动正常；缩放、活动提醒、点击穿透、状态协议、DeepSeek 等控件可操作 | 通过 |
| A6 | 切换宠物 | 本地库、`~/.codex/pets`、`~/.unipet/pets` 均能列出；切换后精灵、动画、窗口图标和托盘图标更新 | 通过 |
| A7 | 导入/导出/删除 | 文件夹或 `.zip` 可导入，重复 id 会询问是否覆盖；导出得到 Codex 上传格式；删除本地副本前有确认 | 通过 |
| A8 | 状态协议 POST | `running`、`waiting`、`failed`、`review`、`waving`、`jumping`、`running-left`、`running-right` 能切换动画；带 `message` 时显示气泡；`ttlMs` 到期回 base | 自动化通过（协议/TTL）；动画与气泡视觉待人工 |
| A9 | 状态协议 GET | `GET /health` 返回当前 pet/persona/state；`GET /pets` 返回宠物 id 列表 | 自动化通过 |
| A10 | 重启持久化 | 当前宠物、人格、缩放、穿透、活动提醒、状态协议端口等写入 `config.json`，重启后保持 | 通过 |
| A11 | 托盘隐藏/显示/退出 | 隐藏后宠物消失且不再响应桌面点击；托盘可恢复；退出后进程真正结束，锁文件可在下次启动时重新获取 | 通过；隐藏/显示状态与退出重启已自动化 |
| A12 | DeepSeek 凭据 | 保存 API key 后能在 Windows 凭据管理器中找到 Petsona 条目；重启后仍可读取；无 key 时回落到固定/时段问候 | 通过 |
| A13 | 活动提醒（长测） | 启动后等待首次约 45 分钟，确认宠物自动播放移动并显示提醒气泡；完成一轮后按设置间隔；用户交互后不会打断当前动作 | 加速计时自动化通过；真实 45 分钟与视觉观感待人工 |

## B. Windows 交互后端验收

| # | 操作 | 预期结果 | 当前状态 |
|---|---|---|---|
| B1 | 左键单击精灵 | 播放挥手并显示气泡；320ms 内的点击不能被误判成单击 | 通过 |
| B2 | 快速双击精灵 | 播放一次跳跃；不会先播一次挥手 | 通过 |
| B3 | 按住拖动 | 宠物跟手移动；左右移动时播放 running-right/left；松手立即停下且不触发单击 | 通过 |
| B4 | 右键精灵 | 自绘菜单出现在光标位置；按 Esc 或点击菜单外关闭；菜单不被宠物窗口裁切 | 通过 |
| B5 | 光标移到宠物左右两侧 | row9/row10 各播放一次转头动作；0.9s 冷却；正前方死区不触发 | 通过，但是需要改成鼠标未移除触发区域是，宠物需要一直看着鼠标的方向。 |
| B6 | 开启“像素级点击穿透” | 透明像素的点击落到桌面；精灵不透明像素仍能单击/双击/拖动；关闭后整个精灵矩形可交互 | 通过 |
| B7 | 在记事本/浏览器输入时点宠物 | 前台应用不失去焦点，继续接收键盘输入；宠物仍能响应鼠标动作 | Win32 `MA_NOACTIVATE` 守卫与前台不变检查自动化通过；真实点击注入在当前会话不可用，最终焦点待实机确认 |
| B8 | 单击、右键、打开设置、改尺寸 | 宠物周围不出现标题栏、白边或边框闪烁；设置窗口本身可以正常获得焦点 | 自动化通过：frame style 不重建；缩放改为原子 geometry，保持底部中心锚点；命中区域按 scale 换算；肉眼观感待最终确认 |
| B9 | 发送带 message 的状态 | 气泡完整显示、不被裁切；出现/消失时宠物不移动、窗口不跳、不闪 | 通过；窗口几何稳定已自动化，裁切/闪烁待人工 |
| B10 | 打开托盘菜单后等待并点击退出 | 菜单打开期间宠物动画仍继续；退出项能立即执行；程序不冻结、不残留菜单窗口 | 通过 |
| B11 | 空闲时看任务管理器 | Petsona 空闲 CPU/GPU 接近 0–1%，允许偶发波动；不持续 60 FPS 重绘 | 事件驱动鼠标唤醒 + 光标缓存已实现；非交互会话跳过 CPU 测量，真实桌面 CPU/GPU 待实机确认 |
| B12 | 启动第二个同数据目录实例 | 不出现第二只宠物；第二个进程退出或只保留一个托盘图标 | 通过 |
| B13 | 查看 `logs\petsona.log` 并重启 | 启动、托盘、状态事件等关键日志写入文件；进程退出后重启不被旧锁阻挡 | 通过 |
| B14 | 退出后重新启动 | 宠物、设置和状态服务恢复正常；没有残留窗口、托盘图标或端口占用 | 通过；quit/restart 已自动化 |

## C. Windows 窗口、多屏与系统集成

这些项目验证 Windows 特有的 Win32/DWM 行为。C1–C3 与 Phase 2 的位置记忆和多屏落位直接相关；
在 Phase 2 完成前，失败可以记录为“已知待实现”，但不要误报为当前基线回归。

| # | 操作 | 预期结果 | 当前状态 |
|---|---|---|---|
| C1 | 把宠物拖到副屏 | 位置正确，不跳回主屏；缩放和点击命中位置不漂移 | 通过 |
| C2 | 在副屏右键托盘/宠物 | 菜单出现在光标所在显示器，并被夹在该显示器工作区内 | 待实现（Phase 2） |
| C3 | 拖动后重启 | 精确还原到上次位置；保存的显示器不存在时回落到可见显示器 | 待实现（Phase 2） |
| C4 | 100%/125%/150%/200% DPI 或混合缩放 | 精灵清晰、逻辑尺寸合理；透明像素穿透和不透明像素命中位置一致 | 待实测 |
| C5 | 全屏应用、任务栏自动隐藏、虚拟桌面 | 置顶层级符合预期；点击宠物不抢前台；切换虚拟桌面后隐藏/显示状态一致 | 待实测 |
| C6 | 用 Win32 自省窗口 | `Petsona` 为 `WS_POPUP`、无 `CAPTION`、带 `WS_EX_NOACTIVATE`；`Petsona 菜单` 同样不激活且无边框；Phase 1 完成后增加对 `Petsona 气泡` 的检查 | 宠物窗口和菜单自动化通过；独立气泡待 Phase 1 |

窗口样式自查命令参考 `AGENTS.md` 的「关键事实速查」；预期宠物样式至少满足：

```text
POPUP=True
CAPTION=False
NOACTIVATE=True
```

## D. Windows 打包与发布

当前仓库还没有 Windows 发布脚本和图标，以下条目属于 Phase 4 的验收目标：

| # | 操作 | 预期结果 | 当前状态 |
|---|---|---|---|
| D1 | 运行 release `target\release\petsona.exe` | 不打开控制台窗口；窗口、托盘、日志和退出行为与 debug 一致 | 待实测 |
| D2 | 检查 exe 图标 | 资源管理器、任务栏和快捷方式显示 `Petsona.ico`，不是默认 Rust 图标 | 待实现 |
| D3 | 运行 `scripts\package-windows.ps1` | 生成可解压运行的 zip，包含 exe、必要资源和版本信息 | 待实现 |
| D4 | 触发 Windows release workflow | GitHub Actions 生成架构包，产物可下载 | 待实现 |
| D5 | 启用/关闭开机自启 | 写入/删除 HKCU Run 或 Startup 项，下一次登录行为正确 | 待实现 |
| D6 | 在干净 Windows 用户环境启动 | 不依赖开发目录；首次启动能建立数据目录、日志和宠物库 | 待实测 |

## E. 记录模板

| 日期 | Windows 版本 | 架构/工具链 | 构建方式 | 结论 | 备注 / 日志 |
|---|---|---|---|---|---|
|  |  |  | `cargo run -p petsona-app` |  |  |
|  |  |  | `target\release\petsona.exe` |  |  |

`待实测` 是默认状态，不是失败；只有实际执行并观察到结果后才改为 `✅ 已验证`。
`待实现` 则表示功能尚未落地，不应写入当前 Windows 基线通过结论。

## F. 自动化验收路线（第一步已实现）

本节记录自动化落地状态；当前第一步已实现，其余条目是后续计划。目标是让每次代码改动只跑快速门禁，
窗口/协议类回归交给可重复的 Windows smoke，人工只保留视觉判断、真实桌面交互、
多屏和发布环境这类无法可靠脚本化的项目。实现只使用 PowerShell/Win32 和现有
`windows-sys`，不新增第三方依赖。

### F.1 每次改动：快速门禁

保留当前 `scripts\verify-windows.ps1` 作为快速入口，覆盖：

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `cargo build --workspace --release --locked`

`test-hooks` 已落地；Full 模式会对 `petsona-app --features test-hooks` 跑 clippy、测试和 release 构建，避免仅测试模式启用的代码腐化。

纯逻辑必须先进入单元测试，例如宠物状态机、持续注视状态、菜单工作区夹取、位置持久化、
自动行走和配置序列化。快速门禁应控制在几分钟内，不启动 GUI。

### F.2 Windows smoke：自动启动真实程序

`scripts\windows-smoke.ps1` 已实现，由 `verify-windows.ps1 -Full` 调用。它负责：

1. 使用临时 `PETSONA_HOME` 和测试配置，避免污染真实数据。
2. Full 模式启动带 `test-hooks` 的 release；默认 release smoke 仍可独立运行。
3. 通过 `EnumWindows`、`GetWindowLongPtrW`、`GetWindowRect` 等 Win32 API 检查窗口存在性、
   样式、位置、可见性和进程状态；焦点检查待后续 SendInput 自动化。
4. 通过状态协议验证 `POST /state`、`GET /health`、`GET /pets` 和 TTL；通过
   `test-hooks` 加速自动行走并确认窗口移动。
5. 用 `SendInput`/`SetCursorPos` 自动执行单击、双击、右键、拖拽、透明像素/不透明像素点击、
   持续注视和焦点回归。
6. 用进程 CPU 采样、内部重绘计数和指针轮询计数判断空闲功耗回归。
7. 启动第二个同数据目录实例，验证单实例；退出后重启，验证锁和日志。
8. 以 `PASS A1`、`FAIL B7` 这类行输出结果并返回非零退出码；失败时保留日志和临时目录。

当前 `-Full` 已覆盖第 1、3、4、7、8 项，以及通过 `test-hooks` 驱动的菜单、设置、
隐藏/显示、气泡几何、加速自动行走、B7 的 `WM_MOUSEACTIVATE` 守卫和 B8 的窗口样式稳定性；
真实 SendInput 点击、持续注视、功耗和 `-Suite core|perf|all` 等入口留到后续阶段。

### F.3 测试钩子边界

`petsona-app` 已增加编译期 feature `test-hooks`，默认关闭，普通 release 不编译。它只绑定
`127.0.0.1`，要求环境变量提供随机 token，提供：

- 状态快照：窗口可见性、菜单/设置是否打开、气泡文本、当前动画与帧、注视方向、
  穿透状态、窗口位置、logic/UI 次数、状态事件次数、光标轮询次数和样式重应用次数。
- 动作：打开/关闭菜单、打开/关闭设置、隐藏/显示、气泡、缩放、穿透、置顶、
  自动行走、点击/双击、保存和退出。
- 兼容现有状态协议，不改动生产协议和默认行为。

测试钩子只用于把“人工点某个隐藏动作”变成确定性输入；窗口存在、样式、焦点、位置、CPU、
进程退出等关键结论仍由外部 Win32/process 检查，避免测试钩子自证。

### F.4 自动化覆盖目标（当前落地项见开头状态）

| 范围 | 可自动化 | 仍需人工 |
|---|---|---|
| A 基础回归 | A1、A4（内部动作）、A8–A11、A13（加速计时） | A2/A3 托盘图标实际点击、A5/A6/A7 控件与视觉、A12 凭据管理器 |
| B 交互 | B1–B4、B6 的样式切换、B7、B9 的窗口/尺寸稳定性、B10 的事件循环响应、B11、B12–B14 | B6 点击确实落到桌面、B8 的肉眼闪烁、B10 真实托盘菜单、动画观感 |
| C 窗口系统 | C6 Win32 样式 | C1–C5 多屏、DPI、全屏、虚拟桌面 |
| D 发布 | D1、D3/D4 的产物检查 | D2 图标观感、D5 登录后真实自启、D6 干净系统/Defender |

自动化目标不是消灭人工验收，而是把人工从“每次逐条点”缩减为“改动涉及什么，最后只看对应的
视觉和真实桌面项目”。任何自动化通过都不能替代 `B8`、`C` 节和真实发布环境的最终人工签字。

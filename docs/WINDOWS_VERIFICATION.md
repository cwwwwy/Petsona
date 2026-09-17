# Petsona Windows 实机验证清单

这份清单用于在 Windows 上人工验收 Petsona。`scripts\verify-windows.ps1` 只证明代码可以
通过格式、Clippy、测试和 release 链接，不能证明窗口、托盘、穿透和系统集成真的可用；
真正的结论以这份清单为准。

当前已知状态（2026-09-16）：

- ✅ `scripts\verify-windows.ps1` 已通过：`cargo fmt`、`cargo clippy`、工作区测试和
  release 链接均成功。
- ✅ `scripts\verify-windows.ps1 -Full` 已接入 `scripts\windows-smoke.ps1` 和默认关闭的
  `test-hooks`，并自动通过 20 项：T1 控制通道、T2 原生菜单线程、T3 菜单替换/关闭/工作区落位、
  T4 开机自启注册表开关、A1、A4（设置动作，部分）、A8（协议/TTL）、A9、A11（隐藏/显示，部分）、
  A13（加速自动行走，部分）、B7（不抢前台 + 光标注视命中）、B8（窗口样式与缩放稳定）、
  B9（气泡几何，部分）、B11（事件驱动鼠标唤醒）、B12–B14（含退出后屏幕外位置回落）、
  C3（位置记忆存物理像素）、C6（宠物窗口与菜单样式）、C7（重力落到工作区底部）。
- ⚠️ Windows 实机回归已由用户按表更新；未标注“自动化通过”的项目仍以实机结果为准。
- ✅ 阶段 7 的 1/2/3 已落地（2026-09-16）：D5 开机自启开关（HKCU Run，读注册表回填 UI）、
  Phase 2 多屏 + 位置记忆（保存物理像素、菜单按 `MonitorFromPoint` + `rcWork` 夹取、目标显示器
  不存在时回落到最近可见工作区）、Phase 3 重力开关（约 2600 px/s²、上限 1800 px/s，
  落到工作区底部播放一次 `jumping`）。以上都有 smoke 覆盖，仍需要真实多屏/登录后行为人工签字。
- 当前分支 `main` 直接开发；Git 操作由用户手动执行。

最小可用判定：**A1–A3、A8–A11、B1–B11 全部通过**。有副屏条件时，还必须通过 C1–C3；
没有副屏时对应条目标记为“条件不足”，不要用 CI 结果代替。

## 0. 准备

在仓库根目录执行自动门禁：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1
```

脚本失败时先解决 Rust 门禁，再做本清单。`scripts\windows-smoke.ps1` 是 `-Full` 调用的实现文件，只在定位失败时单独运行。
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
cargo run -p petsona-shell-windows
```

- 默认数据目录：`%APPDATA%\Petsona`。
- 覆盖数据目录：`PETSONA_HOME` 指定的目录。
- 日志：`<数据目录>\logs\petsona.log`，同时输出到终端。
- 单实例锁：`<数据目录>\petsona.lock`。
- release 产物：`target\release\petsona-windows.exe`（Windows 外壳二进制，已内嵌 `Petsona.ico`）。
- 便携包：`powershell -ExecutionPolicy Bypass -File scripts\package-windows.ps1`
  → `dist\Petsona-windows-x64-<version>.zip`（含 `Petsona.exe`、`Petsona.ico`、`VERSION.txt`、`README.txt`）。
- 图标重新生成（改过 macOS 图标后）：`powershell -ExecutionPolicy Bypass -File packaging\windows\generate-icon.ps1`。
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
| A1 | 启动 `cargo run -p petsona-shell-windows` | 宠物窗口出现；透明背景不是黑底/白底；无标题栏和边框；置顶；任务栏不出现额外宠物窗口 | 通过 |
| A2 | 看系统托盘 | `Petsona` 图标出现；图标使用当前宠物首帧；鼠标悬停显示 `Petsona` | 通过 |
| A3 | 单击/右键托盘图标 | 在光标附近弹出专用线程承载的 Win32 原生菜单；菜单四项为打开设置、更换宠物、隐藏/显示宠物、退出 | 代码已完成；实机待确认 |
| A4 | 点击菜单各项 | 打开设置、切换到宠物页、隐藏/显示宠物、退出分别生效；菜单点击后消失，不留下透明小窗 | 通过；设置开/关已自动化 |
| A5 | 打开设置 | 设置窗口带原生边框，尺寸和滚动正常；缩放、活动提醒、点击穿透、状态协议、DeepSeek 等控件可操作 | 通过 |
| A6 | 切换宠物 | 本地库能列出（含内置 Superintendent）；「从 Codex 导入」能列出并导入 `~/.codex/pets` 里的宠物；切换后精灵、动画、窗口图标和托盘图标更新 | 导入面板为 2026-09-16 新增，待实测 |
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
| B4 | 右键精灵 | Win32 原生菜单出现在光标位置；按 Esc 或点击菜单外关闭；菜单不被宠物窗口裁切 | 代码已完成；实机待确认 |
| B5 | 光标移到宠物左右两侧 | row9/row10 转向后保持最强方向帧；离开触发区播放返回段；0.9s 冷却；正前方死区不触发 | 代码已完成；Windows 实机待验证 |
| B6 | 开启“像素级点击穿透” | 透明像素的点击落到桌面；精灵不透明像素仍能单击/双击/拖动；关闭后整个精灵矩形可交互 | 通过 |
| B7 | 在记事本/浏览器输入时点宠物 | 前台应用不失去焦点，继续接收键盘输入；宠物仍能响应鼠标动作 | Win32 `MA_NOACTIVATE` 守卫与前台不变检查自动化通过；真实点击注入在当前会话不可用，最终焦点待实机确认 |
| B8 | 单击、右键、打开设置、改尺寸 | 宠物周围不出现标题栏、白边或边框闪烁；设置窗口本身可以正常获得焦点 | 自动化通过：frame style 不重建；缩放改为原子 geometry，保持底部中心锚点；命中区域按 scale 换算；肉眼观感待最终确认 |
| B9 | 发送带 message 的状态 | 独立气泡完整显示、不被裁切；出现/消失时宠物不移动、主窗口不跳、不闪 | 自动化通过；裁切/闪烁待人工 |
| B10 | 打开托盘菜单后等待并点击退出 | 专用菜单线程打开期间宠物动画仍继续；退出项能立即执行；程序不冻结、不残留菜单窗口 | 代码已完成；实机待确认 |
| B11 | 空闲时看任务管理器 | Petsona 空闲 CPU/GPU 接近 0–1%，允许偶发波动；不持续 60 FPS 重绘 | 事件驱动鼠标唤醒 + 光标缓存已实现；非交互会话跳过 CPU 测量，真实桌面 CPU/GPU 待实机确认 |
| B12 | 启动第二个同数据目录实例 | 不出现第二只宠物；第二个进程退出或只保留一个托盘图标 | 通过 |
| B13 | 查看 `logs\petsona.log` 并重启 | 启动、托盘、状态事件等关键日志写入文件；进程退出后重启不被旧锁阻挡 | 通过 |
| B14 | 退出后重新启动 | 宠物、设置和状态服务恢复正常；没有残留窗口、托盘图标或端口占用 | 通过；quit/restart 已自动化，并覆盖“屏幕外保存位置回落到可见工作区” |
| B15 | 设置 → 宠物行为 → 重力 | 开启后把宠物拖到半空松手：以约 2600 px/s² 下落（上限约 1800 px/s），停在工作区底部并播放一次 `jumping`；拖动期间和活动提醒行走期间不生效 | 代码与 smoke（C7）通过；手感与观感待人工 |

## C. Windows 窗口、多屏与系统集成

这些项目验证 Windows 特有的 Win32/DWM 行为。C1–C3 的位置记忆和多屏落位已在 2026-09-16
完成并接入 smoke：位置按**物理像素**保存，菜单用 `MonitorFromPoint` + `GetMonitorInfoW(rcWork)`
夹取，保存的显示器不存在时回落到最近可见工作区。真实多屏切换和拔插仍要人工确认。

| # | 操作 | 预期结果 | 当前状态 |
|---|---|---|---|
| C1 | 把宠物拖到副屏 | 位置正确，不跳回主屏；缩放和点击命中位置不漂移 | 通过 |
| C2 | 在副屏右键托盘/宠物 | 菜单出现在光标所在显示器，并被夹在该显示器工作区内 | 已实现；T3 自动检查菜单落在工作区内，副屏实机待确认 |
| C3 | 拖动后重启 | 精确还原到上次位置；保存的显示器不存在时回落到可见显示器 | 自动化通过（C3 + B14：物理像素保存、屏幕外回落到工作区）；混合 DPI 实机待确认 |
| C4 | 100%/125%/150%/200% DPI 或混合缩放 | 精灵清晰、逻辑尺寸合理；透明像素穿透和不透明像素命中位置一致 | 待实测 |
| C5 | 全屏应用、任务栏自动隐藏、虚拟桌面 | 置顶层级符合预期；点击宠物不抢前台；切换虚拟桌面后隐藏/显示状态一致 | 待实测 |
| C6 | 用 Win32 自省窗口 | `Petsona` 和 `Petsona 气泡` 为 `WS_POPUP`、无 `CAPTION`、带 `WS_EX_NOACTIVATE`；原生菜单由专用线程承载 | 宠物窗口和气泡自动化通过；原生菜单实机待确认 |

窗口样式自查命令参考 `AGENTS.md` 的「关键事实速查」；预期宠物样式至少满足：

```text
POPUP=True
CAPTION=False
NOACTIVATE=True
```

## D. Windows 打包与发布

发布脚本、图标和自启开关均已落地；D4 的 workflow 首次执行和 D6 的干净环境仍需实机目标机器确认。

| # | 操作 | 预期结果 | 当前状态 |
|---|---|---|---|
| D1 | 运行 release `target\release\petsona-windows.exe` | 不打开控制台窗口；窗口、托盘、日志和退出行为与 debug 一致 | 待实测 |
| D2 | 检查 exe 图标 | 资源管理器、任务栏和快捷方式显示 `Petsona.ico`，不是默认 Rust 图标 | 已实现（build.rs 内嵌，`ExtractAssociatedIcon` 已验证）；实机 Explorer 观感待确认 |
| D3 | 运行 `scripts\package-windows.ps1` | 生成可解压运行的 zip，包含 exe、图标、README 和版本信息 | 已实现；`verify-windows.ps1 -Full` 会自动检查 zip 结构，解压运行待实机确认 |
| D4 | 打 tag `windows-v<version>` 或手动触发 release workflow | GitHub Actions 跑门禁并上传 zip artifact | workflow 已实现；首次正式产物待验收 |
| D5 | 启用/关闭开机自启 | 写入/删除 HKCU Run 项，下一次登录行为正确 | 已实现（设置 → 启动；T4 自动验证注册表写入/删除与路径）；真实登录后自启待实测 |
| D6 | 在干净 Windows 用户环境启动 | 不依赖开发目录；首次启动能建立数据目录、日志和宠物库 | 待实测 |

## E. 记录模板

| 日期 | Windows 版本 | 架构/工具链 | 构建方式 | 结论 | 备注 / 日志 |
|---|---|---|---|---|---|
|  |  |  | `cargo run -p petsona-shell-windows` |  |  |
|  |  |  | `target\release\petsona-windows.exe` |  |  |

`待实测` 是默认状态，不是失败；只有实际执行并观察到结果后才改为 `✅ 已验证`。
`待实现` 则表示功能尚未落地，不应写入当前 Windows 基线通过结论。

## F. 自动化验收与人工边界

快速门禁与完整 smoke 共用一个入口：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1
powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1 -Full
```

`-Full` 额外检查 `test-hooks` 构建，并调用 `scripts\windows-smoke.ps1` 与打包结构检查。
Smoke 覆盖测试通道、原生菜单线程、状态协议、窗口样式与几何、设置/隐藏显示、单实例、日志、
位置回落和重力等。`test-hooks` 默认关闭，只监听 loopback 并要求 token；它提供可控输入，
窗口和进程结论仍由外部 Win32 检查。

真实托盘/鼠标交互、动画观感、无边框闪烁、真实桌面功耗、多屏/DPI/虚拟桌面、登录后自启和
干净环境运行不能由 smoke 代签。**A–D 表是唯一验收状态来源**；按改动范围勾选对应条目，
不要复制出有独立状态的“本轮 G/H 清单”。

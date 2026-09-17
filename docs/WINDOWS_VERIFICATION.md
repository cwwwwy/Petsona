# Petsona Windows 实机验收清单

自动门禁能证明代码可以编译和通过测试，但证明不了窗口、托盘、穿透和系统集成真的可用；
**真实结论以本清单为准**。`待实测` 是默认状态，不是失败。

最小可用判定：**A1–A3、A8–A11、B1–B11 全部通过**；有副屏时还要过 C1–C3。

## 0. 准备

```powershell
# 先跑完整门禁（fmt / clippy / test / release + 20 项 smoke + 打包结构）
powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1 -Full

# 手动验收建议用独立数据目录
$env:PETSONA_HOME = Join-Path $env:TEMP "petsona-win-test"
cargo run -p petsona-shell-windows
```

MSVC 缺 `link.exe` 时可用 GNU 回退：

```powershell
$env:RUSTUP_TOOLCHAIN = "stable-x86_64-pc-windows-gnu"
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = "$env:USERPROFILE\.rustup\toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\rust-lld.exe"
$env:RUSTFLAGS = "-C link-self-contained=yes"
powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1
```

| 项目 | 位置 / 命令 |
|---|---|
| 数据目录 | `%APPDATA%\Petsona`（`PETSONA_HOME` 可覆盖） |
| 日志 | `<数据目录>\logs\petsona.log`，同时输出到终端 |
| 单实例锁 | `<数据目录>\petsona.lock` |
| release 产物 | `target\release\petsona-windows.exe`（已内嵌 `Petsona.ico`） |
| 便携包 | `scripts\package-windows.ps1` → `dist\Petsona-windows-x64-<version>.zip` |
| 图标重新生成 | `packaging\windows\generate-icon.ps1`（改过 macOS 图标后） |

状态协议（默认端口）：

```powershell
Invoke-RestMethod http://127.0.0.1:17872/health
Invoke-RestMethod http://127.0.0.1:17872/pets
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:17872/state `
  -ContentType 'application/json' `
  -Body '{"source":"win-verify","state":"waiting","message":"Windows 验证","ttlMs":10000}'
```

## A. 基础回归

| # | 操作 | 预期结果 | 状态 |
|---|---|---|---|
| A1 | 启动 `cargo run -p petsona-shell-windows` | 宠物窗口出现；透明背景、无边框、置顶；任务栏无多余窗口 | 通过 |
| A2 | 看系统托盘 | `Petsona` 图标出现，使用当前宠物首帧，悬停显示 `Petsona` | 通过 |
| A3 | 单击 / 右键托盘图标 | 光标附近弹出专用线程承载的 Win32 原生菜单（打开设置 / 更换宠物 / 隐藏显示 / 退出） | 代码完成；实机待确认 |
| A4 | 点击菜单各项 | 四项分别生效；菜单关闭后无残留透明小窗 | 通过；开关动作已自动化 |
| A5 | 打开设置 | 原生边框，尺寸 / 滚动正常；缩放、活动提醒、点击穿透、状态协议、DeepSeek 可操作 | 通过 |
| A6 | 切换宠物 | 本地库列表 +「从 Codex 导入」可用；切换后精灵、动画、窗口 / 托盘图标更新 | 导入面板 2026-09-16 新增，待实测 |
| A7 | 导入 / 导出 / 删除 | 文件夹或 `.zip` 可导入，重复 id 询问覆盖；导出 Codex 上传格式；删除有确认 | 通过 |
| A8 | 状态协议 POST | `running`/`waiting`/`failed`/`review`/`waving`/`jumping`/`running-left`/`running-right` 切换动画；message 显示气泡；TTL 到期回 base | 协议 / TTL 自动化通过；视觉待人工 |
| A9 | 状态协议 GET | `/health` 返回 pet/persona/state；`/pets` 返回 id 列表 | 自动化通过 |
| A10 | 重启持久化 | 宠物、人格、缩放、穿透、活动提醒、协议端口等重启后保持 | 通过 |
| A11 | 托盘隐藏 / 显示 / 退出 | 隐藏后不响应桌面点击；托盘可恢复；退出后进程结束、锁可重取 | 通过；已自动化 |
| A12 | DeepSeek 凭据 | 保存 key 后 Windows 凭据管理器出现 Petsona 条目；无 key 时回落到固定问候 | 通过 |
| A13 | 活动提醒（长测） | 首次约 45 分钟后自动走动 + 气泡，之后按间隔；用户交互不打断当前动作 | 加速计时自动化通过；真实 45 分钟待人工 |

## B. 交互后端

| # | 操作 | 预期结果 | 状态 |
|---|---|---|---|
| B1 | 左键单击精灵 | 挥手 + 气泡；320ms 内不误判双击 | 通过 |
| B2 | 快速双击精灵 | 播放一次跳跃，不先挥手 | 通过 |
| B3 | 按住拖动 | 跟手移动；左右移动播放 running-left/right；松手停下且不触发单击 | 通过 |
| B4 | 右键精灵 | Win32 原生菜单出现在光标处，Esc / 点外关闭，不被宠物窗口裁切 | 代码完成；实机待确认 |
| B5 | 光标移到宠物两侧 | row9/row10 保持最强方向帧；离开播放返回段；0.9s 冷却；正前方死区不触发 | 代码完成；Windows 实机待验证 |
| B6 | 开启像素级点击穿透 | 透明像素点击落到桌面，不透明像素仍可点；关闭后整个精灵矩形可交互 | 通过 |
| B7 | 记事本 / 浏览器输入时点宠物 | 前台不丢焦点，宠物仍响应鼠标 | 自动化 + 真实 SendInput 会话通过（`MA_NOACTIVATE` + 前台不变）；可复测焦点观感 |
| B8 | 单击 / 右键 / 设置 / 改尺寸 | 宠物周围无边框闪；设置窗口打开后前台不跳回旧应用 | frame style 不重建、缩放保持底部中心锚点已自动化；焦点与肉眼观感待人工 |
| B9 | 状态 message / 点击气泡 / 双击宠物 | 独立气泡完整、不裁切；点击气泡打开输入框；输入框只显示单行输入 + ↑；Esc 关闭并缩回宠物影子；开关动画从影子放大 / 缩回 | 独立窗口生命周期、无边框/可聚焦样式已自动化；裁切、闪烁与动画手感待人工 |
| B10 | 打开托盘菜单后等一会再点退出 | 菜单打开时宠物动画继续；退出立即生效；不冻结、不残留菜单窗口 | 代码完成；实机待确认 |
| B11 | 空闲时看任务管理器 | CPU/GPU 接近 0–1%；不持续 60 FPS 重绘 | 事件驱动唤醒已实现；真实桌面 smoke 实测约 0.8% 单核（2026-09-17）；任务管理器观感可复测 |
| B12 | 启动第二个同数据目录实例 | 不出现第二只宠物；第二个进程退出或只留一个托盘图标 | 通过 |
| B13 | 看日志并重启 | 关键日志写入文件；重启不被旧锁阻挡 | 通过 |
| B14 | 退出后重新启动 | 宠物、设置、状态服务恢复正常；无残留窗口 / 托盘 / 端口 | 通过；含屏幕外位置回落 |
| B15 | 设置 → 宠物行为 → 重力 | 半空松手后约 2600 px/s² 下落（上限 1800 px/s），停在工作区底部并播放一次 `jumping`；拖动 / 活动提醒期间不生效 | 代码 + C7 smoke 通过；手感待人工 |

## C. 多屏与系统集成

位置记忆按**物理像素**保存；菜单用 `MonitorFromPoint` + `GetMonitorInfoW(rcWork)` 夹取；
保存的显示器不存在时回落到最近可见工作区。真实多屏切换 / 拔插仍需人工确认。

| # | 操作 | 预期结果 | 状态 |
|---|---|---|---|
| C1 | 把宠物拖到副屏 | 位置正确、不跳回主屏；缩放与命中不漂移 | 通过 |
| C2 | 副屏右键托盘 / 宠物 | 菜单出现在光标所在显示器并被夹在工作区内 | 已实现；T3 自动检查，副屏待实测 |
| C3 | 拖动后重启 | 精确还原；显示器不存在时回落到可见显示器 | C3 + B14 自动化通过；混合 DPI 待确认 |
| C4 | 100% / 125% / 150% / 200% DPI | 精灵清晰、逻辑尺寸合理；穿透与命中一致 | 待实测 |
| C5 | 全屏应用 / 任务栏自动隐藏 / 虚拟桌面 | 置顶层级正确；点击不抢前台；隐藏 / 显示状态一致 | 待实测 |
| C6 | Win32 窗口自省 | `Petsona` / `Petsona 气泡` 为 `WS_POPUP`、无 `CAPTION`、带 `WS_EX_NOACTIVATE`；原生菜单走专用线程 | 宠物窗口 / 气泡自动化通过；菜单实机待确认 |

宠物窗口样式应满足（可用 `EnumWindows` + `GetWindowLongPtrW` 自查）：

```text
POPUP=True  CAPTION=False  NOACTIVATE=True
```

## D. 打包与发布

| # | 操作 | 预期结果 | 状态 |
|---|---|---|---|
| D1 | 运行 release exe | 无控制台窗口；窗口 / 托盘 / 日志 / 退出与 debug 一致 | 待实测 |
| D2 | 检查 exe 图标 | 资源管理器 / 任务栏 / 快捷方式显示 Petsona 图标 | build.rs 内嵌已验证；Explorer 观感待确认 |
| D3 | 运行 `scripts\package-windows.ps1` | 生成可解压运行的 zip（exe / 图标 / README / 版本） | 已实现；结构由 `-Full` 自动检查，解压运行待确认 |
| D4 | 打 tag `windows-v<version>` | `.github/workflows/release-windows.yml` 跑门禁并上传 zip artifact | 待首次执行 |
| D4b | 手动触发 Windows release workflow | GitHub Actions 生成架构包 | 待首次执行 |
| D5 | 启用 / 关闭开机自启 | 写删 HKCU Run 项，下次登录行为正确 | 已实现（设置 → 启动）；T4 自动验证注册表；登录后待实测 |
| D6 | 干净 Windows 用户环境 | 不依赖开发目录；自动建立数据目录 / 日志 / 宠物库 | 待实测 |

## E. 阶段 7 人工确认（H1–H5）

自动化已覆盖 T4（自启注册表）、C3（物理像素位置）、C7（重力落地）、B14（屏幕外回落）。
下面只补“真实系统行为”：

| # | 操作 | 预期结果 | 状态 |
|---|---|---|---|
| H1 | 勾选 / 取消「开机自启动」 | HKCU Run 出现 / 消失 `Petsona` 值，内容为 `"<exe 路径>"`；任务管理器启动应用可见 | 待实测 |
| H2 | 拖到副屏后重启 | 精确回到副屏原位置；混合 DPI 不偏移 | 待实测 |
| H3 | 副屏右键托盘 / 宠物 | 菜单出现在该屏工作区内，不越界、不跳屏 | 待实测 |
| H4 | 拔掉副屏（或把 `startPosition` 改成屏幕外）后重启 | 宠物回到最近可见工作区，不会消失 | 待实测 |
| H5 | 开启重力，半空松手 | 平滑下落、停在工作区底部、播放一次 `jumping`；拖动 / 走动时不下落 | 待实测 |

## F. 自动化覆盖（`-Full`）

20 项 smoke：`T1`–`T4`、`A1` / `A4` / `A8` / `A9` / `A11` / `A13`、
`B7`–`B9` / `B11`–`B14`、`C3` / `C6` / `C7`。

`test-hooks` 默认关闭、只绑 `127.0.0.1` 且要求随机 token；受限会话无法写 HKCU 时 `T4` `[SKIP]`，
无交互桌面时 CPU / SendInput 检查 `[SKIP]`。自动化不能替代 B8 肉眼闪、C4/C5 多屏 / DPI、D6。

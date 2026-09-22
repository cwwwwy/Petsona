> **当前产品线：原生前端（`apps/windows`，C# / WinUI 3 + Win32）。**
> 本文件只维护原生线的准备（0）、人工验收（W）、打包发布（D）与自动化覆盖（F）；
> 旧 egui/Win32 入口的清单已按用户决定冻结并移入
> [`docs/archive/WINDOWS_VERIFICATION-legacy-egui.md`](archive/WINDOWS_VERIFICATION-legacy-egui.md)。
> 执行证据见 [`docs/execution/windows-native-rewrite.md`](execution/windows-native-rewrite.md)。

# Petsona Windows 实机验收清单

自动门禁能证明代码可以编译和通过测试，但证明不了窗口、托盘、穿透和系统集成真的可用；
**真实结论以本清单为准**。`待实测` 是默认状态，不是失败。


## 0. 准备

```powershell
# 先跑完整门禁（fmt / clippy / 95 项测试 / release + 25 项 native smoke + 打包结构）
powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1 -Full

# 原生线手动验收：打包版 exe（或 apps\windows 的 Release 输出），用独立数据目录
$env:PETSONA_HOME = Join-Path $env:TEMP "petsona-win-test"
& "apps\windows\Petsona\bin\x64\Release\net10.0-windows10.0.26100.0\win-x64\Petsona.exe"

# 旧 egui 入口（已冻结，仅供历史对照）
cargo run -p petsona-shell-windows
```

本机环境（2026-09-21 核对）：**VS Build Tools 18（含 C++ 工具）已安装**，`verify-windows.ps1` 因此走
MSVC 的 `petsona_ffi.dll` 分支；GNU 回退仍保留在脚本与下面的手动命令里，两者不要混用同一次构建。

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

## W. 原生前端验收（2026-09-20 起，当前产品线）

自动门禁（本机已验证通过）：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify-windows.ps1 -Full
```

覆盖：Rust workspace 门禁（fmt / clippy / **95 项测试** / release）+ `petsona_ffi.dll` +
dotnet locked restore / build / format / **36 项测试** + **native smoke 25 项**
（N1 启动、N2 宠物窗、N3 宠物加载、N4 窗口样式、N5 编辑按钮、N6 托盘窗口、
N7 协议状态、N8 TTL 回退、N9 穿透切换、N10/N11 单实例、N12 空库首启、
N13 退出释放端口、N14 webp、N15 点击、N16 动画帧变化、N17 托盘 v4 回调菜单、
N18 Composer 聚焦、N19 设置聚焦、N20 拖动动画（按引擎 sprite index）、N21 注视跨行、N22 拖动反向、
N23/N24 窗口接管光标（WM_SETCURSOR 返回 1，且以 NULL 类光标对照窗口返回 0 自校准）、
N25 `action:"clear"` 解除粘滞的协议状态）+ 打包结构与内容检查。Release 构建后还有 FFI DLL SHA256 哈希守卫。

人工验收（需要真实桌面；状态列由人工复测后更新）：

| # | 操作 | 预期结果 | 状态 |
|---|---|---|---|
| W1 | 启动原生 `Petsona.exe`（debug 或打包版） | 宠物窗透明/无边框/置顶、任务栏无多余窗口 | 自动 N2/N4；**2026-09-21 人工通过** |
| W2 | 查看托盘图标并点击 / 右键（含溢出面板与固定到任务栏） | 图标出现；左/右键都弹出五项菜单；Esc / 点外关闭；打开设置不抢焦点循环 | 自动 N6 + N17（v4 `WM_CONTEXTMENU` 回调 → 菜单窗口）；**2026-09-21 人工通过**（溢出面板与固定到任务栏两种状态都验证过） |
| W3 | 单击 / 双击 / 拖动宠物 | 挥手+气泡 / 跳跃 / 跟手移动且松手保存位置；拖动时 running 动画持续前进不被复位 | 拖动与位置持久化端到端通过；runtime 回归 + N20/N22（拖动时引擎帧推进，原地反向立即切 running-left/right）；**2026-09-21 人工通过** |
| W4 | 开启像素穿透后点击透明 / 不透明像素 | 透明处落到桌面；不透明处仍可点 | 自动 N9（样式切换）；**2026-09-21 人工通过** |
| W5 | 光标在宠物附近移动 | 椭圆范围内跟随、静止保持、离开回中性、死区不触发；靠近后 16/33 ms 内开始转身 | 官方 22.5° 映射单测 + GazeStabilizer 迟滞单测 + N21（look-row-9/10 跨行完成）；**2026-09-21 人工通过**（全方向 / 距离 / 抖动均确认） |
| W6 | 气泡 / 编辑按钮 / Composer | 协议 message 显示气泡；按钮打开输入框并聚焦；Enter 发送、Shift+Enter 换行、Esc 关闭且保留草稿；中文 IME 组合不误发 | 打开路径端到端 + N18 聚焦通过；**2026-09-21 人工通过**（Enter 发送 / Esc 草稿 / 中文 IME 均确认） |
| W7 | 设置页各分区 | 导入（文件夹/zip/Codex）/导出/删除/覆盖确认；缩放档位；穿透；DeepSeek 全配置 + 凭据保存；人格 CRUD/导入导出；记忆管理；HKCU 自启 | 命令往返测试 + UI 渲染通过；**2026-09-21 人工通过**（开关写入 / 删除注册表项生效）；**登录后是否真的自启仍待实测（见 D5）** |
| W8 | 清空本地宠物后启动 | 自动打开设置页并聚焦 | 自动 N12 + N19 通过 |
| W9 | 同数据目录启动第二个实例 | 不出现第二只宠物 | 自动 N10/N11 通过 |
| W10 | 托盘菜单退出 | 进程结束、端口释放、无残留窗口/托盘 | 端口释放自动 N13；**2026-09-21 人工通过** |
| W11 | 状态协议 | `/state`、`/health`、`/pets` 与 TTL 语义 | 自动 N7/N8 与 Rust 测试通过；**2026-09-21 人工通过**（5 条步骤含 TTL 回退、`ttlMs:0` 粘滞、非法状态 400 全部符合预期） |
| W13 | 打开 Composer / 设置窗口 | 窗口取得前台与键盘焦点，可直接输入 / 导航 | 自动 N18/N19；**2026-09-21 人工通过** |
| W14 | 冷启动 / 热启动后立即把鼠标移到宠物与编辑按钮上 | 光标是普通箭头，不残留启动期的"启动中"忙碌圈 | 自动 N23/N24（宠物窗与 overlay 都接管 WM_SETCURSOR）；**2026-09-21 人工复测通过**（冷 / 热启动后光标均为普通箭头；启动速度用户确认可接受，CR-W1 选项 C 不改代码） |
| W28 | 记忆来源与隐私 | 事实列表每条显示来源（手动 / 对话 / 导入 / 压缩）；记忆卡片有隐私说明；「清空这只宠物的记忆」只清当前宠物 | 待人工 |
| W27 | 记忆自动：过期与压缩 | 把「事件保留天数」设为 1 后重启：超期事件消失、近期的保留；把「最多偏好」设小并连续添加偏好：旧的合并成一条「画像」而不是消失 | 待人工 |
| W26 | 删除宠物后的数据策略 | 删除一只宠物：绑定消失，但该宠物的人格文件与记忆仍在数据目录（不会被静默删除）；重新导入同一 id 的宠物后能继续使用原说话方式与记忆 | 待人工 |
| W25 | 说话方式：重置 / 导入 / 导出 | 「重置为内置」恢复默认语气与提示词且记忆不变；导出→改风格→导入能还原 | 待人工 |
| W24 | 说话方式与宠物绑定 | 给宠物 A 改语气 → 切到宠物 B（风格为内置/自己的）→ 切回 A（风格保留）；重启后仍绑定；记忆随宠物切换 | 待人工 |
| W23 | 空闲问候位置 | 「外观与交互」页包含启用开关、固定问候文案、空闲分钟、冷却、最大字数；「模型服务」页不再有问候项 | 待人工 |
| W22 | 模型服务卡片顺序与失败提示 | 顺序为 服务商 → Base URL → API Key → 模型 → 高级；断开网络/填错地址后点「拉取模型列表」，模型卡片内直接出现红字失败提示且仍可手填 | 待人工 |
| W21 | 设置页响应式与卡片宽度 | 把窗口拖窄到约 640px：侧边栏收成图标栏；拖宽到 900px 以上：显示文字标签；各页卡片左右边界一致（最宽 1000px） | 待人工（2026-09-22 修复后复测） |
| W20 | 偏好提取不误解问句 | 在输入框问"我叫什么？"，偏好里**不**出现"称呼：什么"；再说"我叫小明"，偏好出现"称呼：小明" | 待人工（2026-09-22 修复后复测） |
| W19 | 拉取模型列表 | 【2026-09-22 复测：清除密钥未生效，已修复，见 W20/W21 同轮记录】点「拉取模型列表」：状态条先显示"正在拉取…"，成功后出现模型下拉（可选中填入，也可手填）；无网络 / 端点不支持时提示失败并保留手填；DeepSeek 与自定义端点各试一次 | 待人工 |
| W18 | 模型服务商 | 选「DeepSeek」：Base URL 自动填内置地址且只读、出现「禁用思考模式」；选「自定义」：Base URL 可编辑、思考开关隐藏并提示不会发送 thinking；两种服务商各自保存/清除密钥互不影响 | 待人工 |
| W17 | 记忆页 | 选中一条偏好 → 改内容 → 「更新」就地生效（不新增重复条）；「只清偏好」保留事件、「只清事件」保留偏好、「全部清空」两者皆清；导出后用「导入记忆…」覆盖回来（导入前有确认） | 待人工 |
| W16 | 外观与交互页缩放 | 拖动滑块：窗口实时缩放并吸附 50% / 75% / 100% / 125% / 150% / 175% / 200%，百分比标签同步，重启后保持 | 待人工 |
| W15 | 设置页收束（settings-consolidation） | 六页卡片布局（宠物 / 外观与交互 / 人格 / 记忆 / 连接与问候 / 系统）；改动即时生效（无保存按钮）；删除宠物 / 清空记忆有确认；系统页可打开数据与日志目录、显示版本 | 自动：dotnet 36/36（含 `UpdateGreetingConfig`）、`verify-windows.ps1 -Full` exit 0、smoke 25/25 ×2；**2026-09-21 用户实机通过**（后续设置页微调见执行记录「下一批」） |
| W12 | 多屏 / 混合 DPI / 工作区夹取 / 重力 / 自动活动提醒 / 透明度 / 协议设置界面 / 宠物图标托盘化 / 影子动画 | —— | **暂缓**（计划 v1.1 §3.1，与 macOS 线一致；解冻需用户确认） |

### W11 手工步骤（状态协议，隔离实例）

验收包入口 `启动隔离验收.cmd` 使用隔离 home `%TEMP%\petsona-acceptance2`，协议端口是 **17873**（17872 是日常实例）；以下命令在 Windows PowerShell 里逐条粘贴。协议本身是给 Codex / 其他工具推送宠物状态的本地接口，`POST /state` 的 `message` 会以气泡显示。

```powershell
# 1 健康检查：返回 pet / persona / state 等字段
Invoke-RestMethod http://127.0.0.1:17873/health | ConvertTo-Json -Depth 5

# 2 宠物列表：返回本地库 id 数组（如 boba）
Invoke-RestMethod http://127.0.0.1:17873/pets

# 3 TTL：等待 10 秒后自动回到 idle，气泡消失
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:17873/state -ContentType 'application/json; charset=utf-8' `
  -Body '{"source":"win-verify","state":"waiting","message":"Windows 验证","ttlMs":10000}'

# 4 ttlMs 0 = 不过期：一直保持 running，直到被下一条状态覆盖
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:17873/state -ContentType 'application/json; charset=utf-8' `
  -Body '{"source":"win-verify","state":"running","message":"不过期","ttlMs":0}'

# 5 非法状态名：应打印 HTTP 400，宠物状态不变
try { Invoke-RestMethod -Method Post -Uri http://127.0.0.1:17873/state -ContentType 'application/json; charset=utf-8' -Body '{"state":"nope"}' }
catch { "HTTP " + [int]$_.Exception.Response.StatusCode }
```

（`charset=utf-8` 是给 Windows PowerShell 5.1 的：不带它时中文气泡可能变乱码；PowerShell 7 无此问题。）

判定：第 3 条宠物切到 waiting 动画、气泡显示「Windows 验证」，约 10 秒后回 idle；第 4 条保持在 running 不过期；第 5 条打印 `HTTP 400`。放行状态名：`idle` / `running` / `waiting` / `failed` / `review` / `waving` / `jumping` / `running-left` / `running-right`（`waving` / `jumping` 为一次性动作，播完回 fallback）。

**第 4 条之后怎么解除粘滞**：协议状态按 `source` 归属，进入引擎后是 `hook:<source>`：

- **同一 source 必然覆盖自己**（不看优先级）：`Invoke-RestMethod -Method Post -Uri http://127.0.0.1:17873/state -ContentType 'application/json; charset=utf-8' -Body '{"source":"win-verify","state":"idle","ttlMs":0}'` —— 立刻回到 idle；注意必须带原来那个 `source`，省略时默认 `hook`，`hook:hook` 和 `hook:win-verify` 是两个 source，`idle` 优先级 0 顶不掉 `running`；
- 换 source 时必须**严格更高**优先级才顶得掉：`failed` 90 > `waiting` 80 > `running` 70 > `review` 60 > `waving`/`jumping` 40 > look 行 20 > `running-left/right` 10 > `idle` 0（优先级相同也拒绝，后来者输）；
- 用户交互**顶不掉**协议状态：点击是 `native` 源的 `waving`（40）、拖动是 `running-left/right`（10），都低于 `running` 的 70；Composer 聊天只发对话、不推状态。这是既定设计（用户交互不打断 agent 状态）；
- 重开应用也能解除（覆盖状态只在内存中，不持久化）。

若某个 hook 推了 `ttlMs:0` 之后没有再推结束状态，宠物会一直保持该状态。**协议侧解除入口（CR-W2 选项 A，2026-09-21 实现）**：同一 source 发一条 `action:"clear"` 即可，不需要 `state`：

```powershell
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:17873/state -ContentType 'application/json; charset=utf-8' -Body '{"source":"win-verify","action":"clear"}'
```

要点：`clear` 只解除**该 source 自己**的覆盖（其他 source 的状态不受影响）；同一 body 里既带 `state` 又带 `action:"clear"` 时 **clear 优先**；`action` 匹配会去掉首尾空白且大小写不敏感；未知 `action` 不改变原逻辑（带合法 `state` 时照常推状态，否则仍 400）。自动覆盖：smoke **N25**。

## R. 发布前清单（windows-v0.1.0-rc.1，2026-09-22）

| # | 事项 | 状态 |
|---|---|---|
| R1 | 版本号规则：SemVer，唯一来源 `Cargo.toml` workspace `version`；tag `windows-v<version>`；预发布 `-rc.N` | ✅ 已定为 `0.1.0-rc.1`（`Cargo.lock` 同步） |
| R2 | 运行时依赖：**self-contained**（`WindowsAppSDKSelfContained` + `SelfContained`）→ 解压即用，无需装 .NET / Windows App SDK | ✅ csproj 已开；本地副本构建成功（238 MB 目录，含 coreclr / WindowsAppRuntime）；自包含产物 **smoke 25/25** |
| R3 | UNC 限制：`mt.exe` 不能读 `\\wsl.localhost\...`，因此 UNC 根会回退为 framework-dependent 并打印警告 | ✅ `verify-windows.ps1` / `package-windows.ps1` 已处理；**自包含 zip 必须在本地副本或 CI 生成** |
| R4 | Release 通道（W-28）：tag 触发 `gh release create`，`-rc.` 自动标记 pre-release | ✅ `release-windows.yml` 新增步骤 |
| R5 | CHANGELOG / README：0.1.0-rc.1 条目 + 安装 / 升级 / 卸载 / 隐私说明 | ✅ 已写 |
| R6 | FFI panic 隔离（REV-05）：`catch_unwind` 已存在；新增回归测试，并修掉"panic 文案被 null-handle 错误覆盖" | ✅ `cargo test -p petsona-ffi` 4/4 |
| R7 | 完整门禁 | ✅ `-Full`：cargo 全量 + dotnet 37/37 + 打包结构通过；native smoke 首轮 N18 FAIL（桌面鼠标干扰）→ 单独重跑 **25/25** |
| R8 | D1/D2 release exe 无控制台、任务栏 / 资源管理器图标 | ⏳ 待人工（用自包含产物） |
| R9 | D5 开机自启：真实注销 + 登录验证 | ⏳ 待人工 |
| R10 | D6 干净机器（或干净 Windows 用户）：解压 → 双击 → 能用 | ⏳ 待人工（自包含产物已就绪，见交付说明路径） |
| R11 | 首次 `workflow_dispatch` 试跑 release workflow | ⏳ 待人工（推送后） |
| R12 | 打 tag `windows-v0.1.0-rc.1` → CI 出包 → 检查 Release（pre-release） | ⏳ 待人工（命令见执行记录） |

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

## F. 自动化覆盖（`-Full`）

**原生线（当前产品线）**：`scripts\windows-smoke.ps1` **25 项** `N1`–`N25`（启动 / 窗口 / 宠物 /
样式 / 编辑按钮 / 托盘 / 协议 / TTL / `action:clear` / 穿透 / 单实例 / 空库首启 / 端口释放 /
webp / 点击 / 动画帧 / 托盘菜单 / 聚焦 / 拖动动画 / 注视 / 光标），随后是打包结构检查。

运行注意：smoke 使用隔离 `PETSONA_HOME` 与空闲端口，但点击 / 拖动 / 注视类用例会**真实操作物理鼠标**——
跑 `-Full` 或 `windows-smoke.ps1` 时不要同时使用鼠标，否则 smoke 会自报「物理光标被另一输入设备移动」
（N15 `[SKIP]`）并连带 N18/N20/N21 假失败；这类失败重跑即可。旧 egui 入口的 20 项 smoke 清单见归档文档。

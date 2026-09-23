# Petsona macOS 实机验收清单

> 原生前端迁移入口：`apps/macos/Petsona.xcodeproj`。Rust FFI/SwiftUI/AppKit
> 的构建契约和迁移范围见 `docs/plans/native-ui-rewrite.md`；本清单中的旧 `cargo run`
> 入口在原生功能全部对照完成前仅用于回归。

> 下文历史通过项针对旧 Rust/egui 外壳，不能记为新原生实现已通过。
> 新任务要求见 [计划](plans/native-ui-rewrite.md)，当前未完成项与审查证据见
> [执行记录](execution/native-ui-rewrite.md)。当前完整脚本已经切换到原生 `.app`
> 的 build、XCTest、smoke 与打包检查；窗口视觉和真实桌面行为仍必须人工确认。

CI 只能证明“能编译”，不能证明“能用”；**真实结论以本清单为准**。
最小可用判定：**B1（左键）、B3（拖拽）、B5（转头）、B6（穿透）、B11（空闲 CPU）全部通过**。

## 已知状态（2026-09-23）

- ⚠️ 2026-09-23 已生成整体验收包 `dist/Petsona-macos-arm64-acceptance/`（应用 + 固定 `acceptance-data/`）及干净初始数据 zip；目录、签名结构、重打包保留数据、zip 解压与正式包排除检查通过。早先一次独立启动出现 AppKit `Abort trap: 6` / `open` 返回 `kLSNoExecutableErr`；随后最新验收包直接 smoke 重试通过 7/7（E-30d）。因此协议级启动已通过，但 Finder/`open` 启动观感、窗口视觉和真实 GUI 交互仍需人工确认。执行证据见 [macOS 执行记录](execution/native-ui-rewrite.md) 8.21、8.22。
- ✅ 2026-09-23 第一批 Windows 对齐代码已接入：气泡 150ms 淡入、剩余时间进度条、悬停暂停/恢复；编辑入口固定为可点击圆角条并支持 220ms 横向/纵向展开、工作区边缘侧挂；Composer 可按下方/左侧/右侧空间跟随宠物；计划暂缓的活动提醒/重力开关不再显示在 macOS 设置页。自动验证见执行记录 8.22；气泡观感、触控板悬停、边缘侧挂和 IME 仍需人工复验。
- ✅ 2026-09-23 第二批设置对齐已接入：六页顺序、统一卡片/右侧控件列、自适应横排/竖排控件、720px 最小窗口和遗留人格 CRUD 状态清理；启动凭据检查改为异步，不再阻塞状态协议。Release 编译通过；最终布局视觉仍待人工复验。
- ✅ 2026-09-23 代码审查追修自动部分已接入：验收启动改为复用实例并通过 reopen 回调重新聚焦设置；验收专用环境标记确保首次启动无论宠物库是否为空都打开设置。设置页改用单一原生标题栏 + 内容区页标题、移除强制内容最小宽度；悬停只展开编辑入口；凭据探针按请求序号丢弃过时结果；气泡淡入按帧推进；Composer 宽度受侧边可用空间限制。最终完整门禁 core 71、FFI 4、runtime 15、XCTest 20/20、smoke 7/7 通过；窗口焦点、布局与悬停仍需人工验收。
- ✅ 2026-09-22 近期设置收束 / 一宠一人格共享改动和审查修复后，`bash scripts/verify-macos-all.sh` 完整通过：Rust core 71、FFI 4、runtime 14；原生 XCTest 14/14；native smoke 7/7；Release 静态依赖、arm64 与包结构检查通过。XCTest Host 使用临时 home、关闭协议端口，完整门禁确认真实用户 config / 日志 / 锁元数据未变化。证据见 [macOS 执行记录](execution/native-ui-rewrite.md) 8.20。
- ✅ 已通过：A3、B1–B4、B6–B7；透明穿透、托盘菜单、设置聚焦、当前 Space 稳定。
- ✅ 2026-09-17 完整 `verify-macos-all.sh` 通过；34 项 runtime smoke 含 LaunchAgent plist 开 / 关。
- ✅ 2026-09-18 `bash scripts/verify-macos-all.sh` 通过：fmt / clippy / workspace tests（app 19、core 59、runtime 1、macOS shell 7）/ release、33 项 runtime smoke、打包结构检查。
- ✅ 2026-09-19 原生重构门禁通过：workspace Rust tests（app25/core54/ffi3/runtime3/mac shell7）、原生 XCTest 4/4、原生 app FFI smoke（6 项）、静态 `.a` 依赖检查、arm64 bundle/zip 检查。
- ✅ 2026-09-20 新增配置/记忆/人格命令往返 XCTest 后，原生测试摘要为 5/5 通过。
- ✅ 2026-09-20 短期功能批次代码与自动测试：DeepSeek 全配置、记忆事实/事件、人格 CRUD/模板/导入导出、偏好提取、重复宠物覆盖确认、拖放导入、固定缩放档位已接入 worker/原生设置与状态栏菜单；Rust runtime/FFI 测试、原生设置命令往返测试通过。
- ⚠️ 当前 native smoke 只覆盖协议/TTL/进程退出；窗口视觉、点击穿透、IME、托盘、LaunchAgent 登录行为仍需人工。
- ⚠️ 当前会话没有可枚举的 winit 显示器，NSScreen 工作区与设置 Key Window 两项明确 `[SKIP]`；几何单测通过，真实桌面行为待确认。
- ⚠️ 待复测：B5 真实光标、B8 缩放视觉、B9 输入框动画 / 注视、B11 Activity Monitor（修复前 8–10%）。
- ⏳ 待实机：B10 多屏、B12/B13、C 节 Retina / Spaces、D 节真实签名 / 公证。
- ℹ️ 位置记忆存**物理像素**；工作区由 `NSScreen.visibleFrame` 提供，排除 Dock / 菜单栏后用于位置夹取和重力落点。
- ℹ️ 设置页现可创建 / 移除当前用户的 LaunchAgent；真实注销后登录启动待实机确认，脚本安装方式仍保留。

## 0. 准备

```bash
xcode-select --install            # 只需一次
cargo test --workspace
bash scripts/package-macos.sh
cd "dist/Petsona-macos-$(uname -m)-acceptance" && open --env "PETSONA_HOME=$PWD/acceptance-data" --env "PETSONA_AUTOSTART_PLIST_DIR=$PWD/acceptance-data/LaunchAgents" --env "PETSONA_OPEN_SETTINGS_ON_LAUNCH=1" "$PWD/Petsona.app"
```

- 默认数据目录：`~/Library/Application Support/Petsona/`；日志写入 `logs/petsona.log` 和终端
  （`RUST_LOG=debug` 可调级别）。
- 人工验收包为 `dist/Petsona-macos-<架构>-acceptance/`，包含 `Petsona.app` 与同级 `acceptance-data/`；同一目录重复打包会保留验收数据，不会每次生成新的临时目录。压缩包为 `dist/Petsona-macos-<架构>-acceptance.zip`，解压后也是完整目录。
- 首次启动隔离包前，先从菜单栏退出日常运行的 Petsona；两者 bundle id 相同，避免 LaunchServices 将首次启动路由到日常实例。命令为验收实例设置隔离 home 和 `PETSONA_OPEN_SETTINGS_ON_LAUNCH=1`，即使库中已有宠物也会打开设置；命令不带 `-n`，重复运行会复用验收实例并通过 reopen 回调重新聚焦设置窗口。请实际运行两次，第二次应聚焦设置窗且不创建第二个进程。
- 必须通过上方 Terminal 命令启动验收包；Finder 双击 `.app` 不会继承 `PETSONA_HOME`，可能写入日常 `~/Library/Application Support/Petsona/`。
- 隔离 home 初始为空宠物库、关闭状态服务（端口预留 17873）；首次启动会进入空库设置，宠物只在用户手动导入后进入该验收目录。配置、日志、宠物库和单实例锁均留在 `acceptance-data/`。
- 启动命令将 LaunchAgent plist 重定向到 `acceptance-data/LaunchAgents`，用于避免改动日常登录自启；它**不验证真实登录后自启**。macOS 钥匙串仍使用正式服务标识 `com.petsona.desktop`；在隔离包中不要保存或清除 API Key，以免影响日常应用凭据。
- 验收结束后删除整个 `Petsona-macos-<架构>-acceptance/` 文件夹即可清理本包数据。打包脚本生成的压缩包始终使用干净初始数据，不会把已有验收记录或导入的宠物打进去。

## A. 基础回归

| # | 操作 | 预期结果 | 状态 |
|---|---|---|---|
| A1 | 启动 | 宠物窗口出现、无边框、透明背景、置顶 | 待实测 |
| A2 | 看菜单栏 | 出现 Petsona 托盘图标 | 待实测 |
| A3 | 点击托盘图标 | macOS 原生菜单：打开设置 / 选择宠物（V2 有勾选）/ 显示隐藏 / 退出 | 代码完成；实机已通过一次，可复测 |
| A4 | 打开设置 | 能打开、滚动；页面顺序为宠物库 / 外观与交互 / 人格 / 记忆 / 模型服务 / 系统；原生标题栏、侧边栏和内容区标题不重复/错位；在约 720、900、1000px 宽度检查侧边栏与自适应卡片，控件不得挤压、错位；缩放档位、穿透、DeepSeek、人格、记忆可操作 | Release 编译通过；窗口视觉/交互待人工 |
| A5 | 切换宠物 | 本地库 + 原生导入 / 切换可用；切换后动画与窗口更新 | native worker/UI 已接；人工待实测 |
| A6 | 导入 / 导出 | 文件面板或拖放导入；Codex 可预览；重复 ID 明确确认覆盖；保存面板导出；删除有确认 | native worker/UI 已接；人工待实测 |
| A7 | 状态协议 POST | `waiting`/`failed`/`review`/`running` 切换动画；message 显示气泡；TTL 回 base | 待实测 |
| A8 | 状态协议 GET | `/health`、`/pets` 返回正确 | 待实测 |
| A9 | 重启持久化 | 宠物、人格、缩放、位置写入 `config.json` 并保持 | 位置写入 smoke 通过；真实拖动重启待人工 |
| A10 | 托盘隐藏 / 显示 / 退出 | 隐藏后窗口消失、可恢复；退出后进程真的结束 | 待实测 |
| A11 | DeepSeek / Keychain | Base URL、模型、超时、token、温度、思考模式可保存；Keychain 密钥可保存/清除，重启仍可读；无 key 回落固定问候 | 自动接线通过；待实测 |
| A12 | 设置 → 启动 → 开机自启动 | 勾选 / 取消会创建 / 移除 `~/Library/LaunchAgents/com.petsona.desktop.plist` | 原生服务 XCTest 隔离验证；真实设置控件/登录启动待实测 |

```bash
curl -s http://127.0.0.1:17872/health
curl -s http://127.0.0.1:17872/pets
curl -XPOST http://127.0.0.1:17872/state \
  -H 'content-type: application/json' \
  -d '{"source":"mac-verify","state":"waiting","message":"macOS 验证","ttlMs":10000}'
```

## B. 交互后端

| # | 操作 | 预期结果 | 状态 |
|---|---|---|---|
| B1 | 左键单击 | 挥手 + 气泡；320ms 内不误判双击 | ✅ 已验证 |
| B2 | 快速双击 | 跳一下，不先挥手 | ✅ 已验证 |
| B3 | 按住拖动 | 跟手；左右播放 running-left/right；松手停下不触发单击 | ✅ 已验证 |
| B4 | 右键 | 原生菜单出现在光标处；点外 / Esc 关闭 | 原生右键菜单已接入；鼠标/触控板实机复验待完成 |
| B5 | 光标在宠物周围移动 | 只在宠物附近的椭圆区域触发；row9/row10 作为方向姿势表，从中性帧逐帧到目标；左右换行先经过对应的上 / 下中间姿势；离开后回中性帧，死区不触发 | 范围与跨行状态机单测通过；真实方向、跟随延迟和过渡观感待复验 |
| B6 | 开启 `click_through` | 透明像素点落到桌面；不透明像素仍可点；关闭后整窗可交互 | ✅ 已验证 |
| B7 | 在 TextEdit / 浏览器输入时点宠物 | 前台焦点不被打断 | ✅ 已验证 |
| B8 | 点击 / 右键 / 设置 / 改缩放 | 无边框闪；设置成为可输入前台窗口；固定档位 `0.5 / 0.75 / 1.0 / 1.25 / 1.5 / 1.75 / 2.0` 在设置和状态栏菜单可选且不闪 | 缩放锚点 / 几何 smoke 自动通过；Key Window 需在有显示器的桌面确认，缩放视觉待复测 |
| B9 | 状态 message / 编辑按钮 / 输入框 | 气泡点击打开 Composer；编辑入口悬停只展开、不打开输入框或抢焦点，点击才打开；Composer 使用 NSTextView，Enter/Shift+Enter/Esc、草稿、caret 注视；输入框按工作区在宠物下方或左右侧挂且不得压住宠物 | 原生编译、布局/XCTest 与 worker smoke 通过；IME、悬停时序、位置和视觉待实测 |
| B10 | 在副屏右键 | 菜单出现在该屏并夹在工作区内 | 待实测（当前无副屏条件） |
| B11 | 空闲看 Activity Monitor | CPU 接近 0–1%（允许偶发波动） | 修复前 8–10%；低频采样 smoke 通过，待复测 |
| B12 | 启动第二个实例 | 不出现第二只宠物；第二进程自动退出 | native smoke 已验证第二实例退出；真实桌面提示观感待实测 |
| B13 | 注销 / 重启后登录 | 宠物自动出现 | 设置与 LaunchAgent 脚本均已实现；真实登录后待实测 |

## C. 多屏 / Spaces / 窗口系统

| # | 操作 | 预期结果 | 状态 |
|---|---|---|---|
| C1 | 拖到副屏 | 位置 / 缩放正确，不跳回主屏 | 暂缓；多屏优先级后置 |
| C2 | Retina + 非 Retina 混用 | 精灵清晰、尺寸不跳、命中不漂 | 暂缓；多屏优先级后置 |
| C3 | 切换 Space / 全屏 App | 置顶行为符合预期（当前 Space 内稳定；不跨所有 Space，暂接受） | 部分通过 |
| C4 | Mission Control / Stage Manager | 不干扰窗口管理，托盘菜单可用 | 待实测 |
| C5 | 深色 / 浅色菜单栏 | 托盘图标两种模式都清晰 | 待实测 |
| C6 | 隐藏 / 显示后 | 位置、层级、穿透状态一致 | 待实测 |

## D. 打包 / 发布

- [x] `cargo build --release` 通过；bundle 结构检查完成
- [x] `scripts/package-macos.sh` 生成 `.app`（Info.plist、bundle id、图标、常规 zip）；本地包 ad-hoc 签名，不含 Developer ID / 公证
- [x] `scripts/package-macos.sh` 生成包含 `.app` + `acceptance-data/` 的整体验收目录与干净初始数据 zip
- [x] `LSUIElement = true`，默认不显示 Dock 图标
- [x] `scripts/install-macos-launch-agent.sh` 安装 / 卸载 LaunchAgent
- [x] `scripts/sign-macos.sh`、`scripts/notarize-macos.sh` 流程就绪（ad-hoc 签名验证过）
- [x] `.github/workflows/release-macos.yml` tag / 手动触发运行完整原生门禁、对最终 dist 包 smoke，并以 `unsigned` 标记产物
- [x] Release workflow 禁止生成并上传本机验收数据包，只上传常规应用包
- [ ] 真实 Developer ID 签名 + 公证 + 目标 Mac 实机验证

> 当前发布工作流故意只生成并上传未签名产物；没有 Developer ID 证书和
> `notarytool` profile 时不得把 CI 绿灯解释为签名或公证通过。

```bash
./scripts/package-macos.sh
cd "dist/Petsona-macos-$(uname -m)-acceptance" && open --env "PETSONA_HOME=$PWD/acceptance-data" --env "PETSONA_AUTOSTART_PLIST_DIR=$PWD/acceptance-data/LaunchAgents" --env "PETSONA_OPEN_SETTINGS_ON_LAUNCH=1" "$PWD/Petsona.app"
./scripts/install-macos-launch-agent.sh install dist/Petsona.app
./scripts/install-macos-launch-agent.sh uninstall

CODESIGN_IDENTITY="Developer ID Application: ..." ./scripts/sign-macos.sh
NOTARYTOOL_PROFILE="petsona-notary" ./scripts/notarize-macos.sh
```

> 注意：2026-09-23 曾出现一次 `open` / AppKit 启动异常；最新整体验收包直接 smoke 重试已通过 7/7，但 `open` 仍不作为自动证据。人工验收仍需使用 Finder 或验收包 README 中的启动命令确认菜单栏、窗口和交互视觉。

## E. 自动化验收

```bash
bash scripts/verify-macos-all.sh               # 门禁 + runtime smoke + 打包结构检查
bash scripts/verify-macos-all.sh --gates-only  # 只跑 fmt / clippy / test / release
```

入口脚本 = workspace Rust 门禁 + 原生 XCTest + 原生 `macos-smoke.sh` +
打包结构检查（可执行文件、图标、Bundle ID、`LSUIElement`、Retina 字段、LaunchAgent 模板）。
smoke 覆盖协议 / TTL、设置、（有可枚举显示器时）Key Window、缩放锚点、位置保存、低频指针、idle 重绘、CPU 趋势、
LaunchAgent plist 开关（使用临时目录，不触碰用户真实登录项）、V2 注视生命周期
（`turning → holding → returning → idle`）。
**不覆盖**：缩放闪动观感、真实触控板 / 托盘点击、菜单外观、Activity Monitor 最终 CPU、
Retina / Spaces、多显示器。

V2 夹具优先用仓库自绘夹具 `crates/petsona-core/testdata/v2-test-pet`，
没有时从 `~/.codex/pets` 搜索并复制进临时 `PETSONA_HOME`
（应用只加载本地库）；也可以显式指定：

```bash
PETSONA_SMOKE_V2_PET_DIR="$HOME/.codex/pets/<pet>" bash scripts/macos-smoke.sh
```

> 2026-09-16 的平台后端搬移只在 Windows 上做过 fmt + 静态审阅（macOS 目标无法在 Windows 上
> 链接检查：`ring` 需要 macOS C 工具链），所以回 mac 后请先跑一次完整脚本。

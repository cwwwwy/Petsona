# Petsona macOS 实机验收清单

> 当前入口是 `apps/macos/Petsona.xcodeproj`（SwiftUI/AppKit，最低macOS26）。
> 当前目标见[桌面陪伴计划](plans/macos-companion-evolution.md)，自动证据与待验项见[执行记录](execution/macos-companion-evolution.md)。
> 旧入口结果保留在历史执行记录中，不代表当前原生入口已经通过人工验收。

## 当前状态（2026-10-04）

- 完整自动门禁通过：Rust core86/FFI9/runtime28、XCTest33/33、native smoke8/8及打包结构；本轮脚本整理后再次通过，证据见执行记录E-11和[结果摘要](execution/macos-companion-evolution-tests.json)。
- `dist/Petsona-macos-arm64-acceptance/` 已更新，重打包保留既有验收数据；模型页已做原生界面观察。完整窗口、IME、注视、Retina/多屏/Spaces及macOS26仍待人工。
- 当前浮层采用原生玻璃、薄胶囊聊天入口和动态多行输入框；高级调参已移除，已有值保留。真实Keychain、登录自启、签名、公证与干净机器验收尚未闭合。
- 本清单只列当前操作。较早构建失败、启动异常、SKIP及修复过程保留在[原生入口执行记录](execution/native-ui-rewrite.md)，不在这里重复展示为当前状态。

## 0. 准备

```bash
# 构建需要完整Xcode；生成工程还需要XcodeGen，构建说明见根README。
bash scripts/verify-macos-all.sh
bash scripts/package-macos.sh
cd "dist/Petsona-macos-$(uname -m)-acceptance"
open --env "PETSONA_HOME=$PWD/acceptance-data" \
  --env "PETSONA_AUTOSTART_PLIST_DIR=$PWD/acceptance-data/LaunchAgents" \
  --env "PETSONA_OPEN_SETTINGS_ON_LAUNCH=1" "$PWD/Petsona.app"
```

- 默认数据目录：`~/Library/Application Support/Petsona/`；日志写入 `logs/petsona.log` 和终端
  （`RUST_LOG=debug` 可调级别）。
- 人工验收包为 `dist/Petsona-macos-<架构>-acceptance/`，包含 `Petsona.app` 与同级 `acceptance-data/`；同一目录重复打包会保留验收数据，不会每次生成新的临时目录。压缩包为 `dist/Petsona-macos-<架构>-acceptance.zip`，解压后也是完整目录。
- 首次启动隔离包前，先从菜单栏退出日常运行的 Petsona；两者 bundle id 相同，避免 LaunchServices 将首次启动路由到日常实例。命令为验收实例设置隔离 home 和 `PETSONA_OPEN_SETTINGS_ON_LAUNCH=1`，即使库中已有宠物也会打开设置；命令不带 `-n`，重复运行会复用验收实例并通过 reopen 回调重新聚焦设置窗口。请实际运行两次，第二次应聚焦设置窗且不创建第二个进程。
- 必须通过上方 Terminal 命令启动验收包；Finder 双击 `.app` 不会继承 `PETSONA_HOME`，可能写入日常 `~/Library/Application Support/Petsona/`。
- 隔离 home 初始为空宠物库、关闭状态服务（端口预留 17873）；首次启动会进入空库设置，宠物只在用户手动导入后进入该验收目录。配置、日志、宠物库和单实例锁均留在 `acceptance-data/`。
- 启动命令将 LaunchAgent plist 重定向到 `acceptance-data/LaunchAgents`，用于避免改动日常登录自启；它**不验证真实登录后自启**。macOS 钥匙串仍使用正式服务标识 `com.petsona.desktop`；在隔离包中不要保存或清除 API Key，以免影响日常应用凭据。
- 自动门禁的 `macos-smoke.sh` 使用独立 `PETSONA_MACOS_SMOKE_API_KEY` 占位环境变量，启动时不会查询系统钥匙串；只有你主动执行 A11 凭据保存/读取验收时才会访问真实 Keychain。
- 验收结束后删除整个 `Petsona-macos-<架构>-acceptance/` 文件夹即可清理本包数据。打包脚本生成的压缩包始终使用干净初始数据，不会把已有验收记录或导入的宠物打进去。

## A. 基础回归

| # | 操作 | 预期结果 | 状态 |
|---|---|---|---|
| A1 | 启动 | 宠物窗口出现、无边框、透明背景、置顶 | 待实测 |
| A2 | 看菜单栏 | 出现 Petsona 托盘图标 | 待实测 |
| A3 | 点击托盘图标 | macOS 原生菜单：打开设置 / 选择宠物（V2 有勾选）/ 显示隐藏 / 退出 | 原生已实现；当前菜单实机待复验 |
| A4 | 打开设置 | 打开时 Dock 显示 Petsona，关闭设置后 Dock 图标消失而菜单栏宠物继续运行；Dock / App 菜单 / Command+, 均回到同一个设置窗口。六页使用 macOS 27 原生侧边栏、分组表单、列表与控件；标题栏分界连续、当前页标题更新；约 720/900/1000pt、浅深色和辅助功能设置下均可读且不遮挡 | E-08 完整自动门禁通过；整体验收按用户要求留到全部任务结束；macOS 26 实机待验 |
| A5 | 切换宠物 | 本地宠物列表单击只选中，双击才切换；键盘选中后可用“切换”按钮；切换后动画、窗口和这只宠物的人格/记忆同步更新 | E-08 自动门禁通过；集中人工验收待执行 |
| A6 | 导入 / 导出 | 文件面板或拖放导入；“从 Codex 导入”扫描入口始终可见、可预览候选，双击候选或选中后点按钮导入；重复 ID 明确确认覆盖；保存面板导出；删除有确认 | E-08 自动门禁通过；集中人工验收待执行 |
| A7 | 状态协议 POST | `waiting`/`failed`/`review`/`running` 切换动画；message 显示气泡；TTL 回 base | 待实测 |
| A8 | 状态协议 GET | `/health`、`/pets` 返回正确 | 待实测 |
| A9 | 重启持久化 | 宠物、人格、缩放、位置写入 `config.json` 并保持 | 位置写入 smoke 通过；真实拖动重启待人工 |
| A10 | 托盘隐藏 / 显示 / 退出 | 隐藏后窗口消失、可恢复；退出后进程真的结束 | 待实测 |
| A11 | DeepSeek / Keychain | 服务商、自定义地址、模型可保存；没有高级调参入口且旧值不被重置；拉取模型时按钮显示“正在拉取…”并禁用，完成/失败后恢复，可手填模型；Keychain 密钥可保存/清除，重启仍可读；无 key 回落固定问候 | 自动接线通过；整体验收后复测 |
| A12 | 设置 → 启动 → 开机自启动 | 勾选 / 取消会创建 / 移除 `~/Library/LaunchAgents/com.petsona.desktop.plist` | 原生服务 XCTest 隔离验证；真实设置控件/登录启动待实测 |

```bash
# A7/A8：退出验收实例，在acceptance-data/config.json中把stateServer
# 设为 {"enabled":true,"port":17873}，确认端口空闲后按上述隔离命令重新启动。
curl -s http://127.0.0.1:17873/health
curl -s http://127.0.0.1:17873/pets
curl -XPOST http://127.0.0.1:17873/state \
  -H 'content-type: application/json' \
  -d '{"source":"mac-verify","state":"waiting","message":"macOS 验证","ttlMs":10000}'
```

## B. 交互后端

| # | 操作 | 预期结果 | 状态 |
|---|---|---|---|
| B1 | 左键单击 | 挥手 + 气泡；320ms 内不误判双击 | 旧入口历史通过；当前原生待复验 |
| B2 | 快速双击 | 跳一下，不先挥手 | 旧入口历史通过；当前原生待复验 |
| B3 | 按住拖动 | 跟手；左右播放 running-left/right；松手停下不触发单击 | 旧入口历史通过；当前原生待复验 |
| B4 | 右键 | 原生菜单出现在光标处；点外 / Esc 关闭 | 原生右键菜单已接入；鼠标/触控板实机复验待完成 |
| B5 | 光标在宠物周围移动 | 只在宠物附近的椭圆区域触发；row9/row10 作为方向姿势表，从中性帧逐帧到目标；左右换行先经过对应的上 / 下中间姿势；离开后回中性帧，死区不触发 | 范围与跨行状态机单测通过；真实方向、跟随延迟和过渡观感待复验 |
| B6 | 开启 `click_through` | 透明像素点落到桌面；不透明像素仍可点；关闭后整窗可交互 | 旧入口历史通过；当前原生待复验 |
| B7 | 在 TextEdit / 浏览器输入时点宠物 | 前台焦点不被打断 | 旧入口历史通过；当前原生待复验 |
| B8 | 点击 / 右键 / 设置 / 改缩放 | 无边框闪；设置成为可输入前台窗口；固定档位 `0.5 / 0.75 / 1.0 / 1.25 / 1.5 / 1.75 / 2.0` 在设置和状态栏菜单可选且不闪 | 缩放锚点 / 几何 smoke 自动通过；Key Window 需在有显示器的桌面确认，缩放视觉待复测 |
| B9 | 状态 message / 编辑按钮 / 输入框 | 气泡点击查看当前宠物的完整聊天历史；编辑入口悬停只展开、不打开输入框或抢焦点，点击才打开；Composer 使用 NSTextView，Enter/Shift+Enter/Esc、草稿、caret 注视；输入框按工作区在宠物下方或左右侧挂且不得压住宠物 | 原生编译、布局/XCTest 与 worker smoke 通过；IME、悬停时序、位置和视觉待实测 |
| B10 | 在副屏右键 | 菜单出现在该屏并夹在工作区内 | 待实测（当前无副屏条件） |
| B11 | 空闲看 Activity Monitor | CPU 接近 0–1%（允许偶发波动） | 修复前 8–10%；低频采样 smoke 通过，待复测 |
| B12 | 启动第二个实例 | 不出现第二只宠物；第二进程自动退出 | native smoke 已验证第二实例退出；真实桌面提示观感待实测 |
| B13 | 注销 / 重启后登录 | 宠物自动出现 | 设置与 LaunchAgent 脚本均已实现；真实登录后待实测 |

## C. 多屏 / Spaces / 窗口系统

| # | 操作 | 预期结果 | 状态 |
|---|---|---|---|
| C1 | 拖到副屏 | 位置 / 缩放正确，不跳回主屏 | 已纳入当前计划；待实机验收 |
| C2 | Retina + 非 Retina 混用 | 精灵清晰、尺寸不跳、命中不漂 | 已纳入当前计划；待实机验收 |
| C3 | 切换 Space / 全屏 App | 置顶与非激活行为符合当前计划，并与锁定Codex基准对照 | 当前原生待复验 |
| C4 | Mission Control / Stage Manager | 不干扰窗口管理，托盘菜单可用 | 待实测 |
| C5 | 深色 / 浅色菜单栏 | 托盘图标两种模式都清晰 | 待实测 |
| C6 | 隐藏 / 显示后 | 位置、层级、穿透状态一致 | 待实测 |

## D. 打包 / 发布

- [x] `cargo build --release` 通过；bundle 结构检查完成
- [x] `scripts/package-macos.sh` 生成 `.app`（Info.plist、bundle id、图标、常规 zip）；本地包 ad-hoc 签名，不含 Developer ID / 公证
- [x] `scripts/package-macos.sh` 生成包含 `.app` + `acceptance-data/` 的整体验收目录与干净初始数据 zip
- [x] `LSUIElement = true`，默认不显示 Dock 图标；打开设置时运行时切换为 Dock 可见，关闭后恢复
- [x] `scripts/install-macos-launch-agent.sh` 安装 / 卸载 LaunchAgent
- [x] `scripts/sign-macos.sh`、`scripts/notarize-macos.sh` 流程就绪（ad-hoc 签名验证过）
- [x] `.github/workflows/release-macos.yml` tag / 手动触发运行完整原生门禁、对最终 dist 包 smoke，并以 `unsigned` 标记产物
- [x] Release workflow 禁止生成并上传本机验收数据包，只上传常规应用包
- [x] macOS `.app` 的 `LSMinimumSystemVersion` 为 26.0；打包和完整门禁检查此值
- [ ] 真实 Developer ID 签名 + 公证 + 目标 Mac 实机验证

> 当前发布工作流故意只生成并上传未签名产物；没有 Developer ID 证书和
> `notarytool` profile 时不得把 CI 绿灯解释为签名或公证通过。

```bash
# 真实登录项验收使用install/uninstall；该工具管理正式用户LaunchAgent。
./scripts/install-macos-launch-agent.sh install dist/Petsona.app
./scripts/install-macos-launch-agent.sh uninstall

CODESIGN_IDENTITY="Developer ID Application: ..." ./scripts/sign-macos.sh
NOTARYTOOL_PROFILE="petsona-notary" ./scripts/notarize-macos.sh
```

## E. 自动化验收

完整入口见准备步骤。`bash scripts/verify-macos-all.sh --gates-only` 运行Rust门禁、原生Release构建和XCTest，跳过运行smoke与打包检查。

- Rust检查：fmt、clippy、workspace测试、release构建。
- XCTest：ABI/生命周期、AppKit资源、显示器几何、注视、配置/人格/记忆往返、文本与浮层布局、隔离LaunchAgent服务。
- native smoke：仅使用仓库自绘V2夹具、临时home与单一空闲状态端口；覆盖凭据隔离、进程/宠物、协议状态、TTL、单实例及退出。`PETSONA_SMOKE_STATE_PORT` 可以指定状态端口。
- 打包：可运行文件、图标、静态依赖、架构、最低macOS26、bundle字段、zip、干净验收包/重打包保留数据和LaunchAgent模板。

测试宿主与smoke在初始化前使用假环境凭据，避免访问日常Keychain；实际凭据保存/清除只在A11手工验收中测试。smoke只加载仓库自绘夹具，不读取用户Codex宠物。

自动检查不替代真实点击/拖动/触控板、IME、菜单焦点、窗口外观、CPU资源趋势、登录自启、Retina/多屏/Spaces、签名或公证验收。

## 2026-10-04 浮层与设置复验

使用最新隔离验收包，在当前系统和 macOS 26 分别记录结果；以下项目仍待人工，不能用自动门禁代替。

- V2 宠物：光标移入身体的上/下/左/右及斜向（Retina 下也要触发），静止能保持，离开能回正；拖动/协议高优先级状态/输入光标注视的切换正确。
- 入口：远离时是薄胶囊，接近后约200ms变为24pt聊天按钮，离开稍后收回；点击才进入本地聊天；下方不足时能侧挂。
- 输入框：原生玻璃与浅/深色系统一致，Enter发送、Shift+Enter换行、中文IME选词不误发；多行自动增高并可滚动；Esc保留草稿；生成中继续编辑并按Enter不会丢稿或重复提交。
- 气泡：玻璃、15pt圆角、可读预览，无彩色尾巴；点击查看当前宠物全文；悬停计时暂停。与锁定 Codex 实机对照材质、尺寸、位置和动作节奏。
- 设置：没有高级token/温度/超时/环境变量、记忆数量/压缩或问候冷却/字数入口；模型连接、人格三种入口、记忆/历史独立控制、宠物管理和必要桌面选项可用。旧调参值不能因修改模型或开关而重置。

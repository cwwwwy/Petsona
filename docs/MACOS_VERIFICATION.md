# Petsona macOS 实机验证清单

这份清单用来在 macOS 上人工验收 Petsona。CI 的 macOS job 只跑 `fmt` / `clippy` / `test`，
只能证明“能编译”，不能证明“能用”；真正的结论以这份清单为准。

当前已知状态（2026-09-13）：

- ✅ 用户已实测通过 A3 以及 B1–B7；B4 的 macOS 原生菜单也已验证。
- ⚠️ B10 暂无副屏验证条件；多显示器坐标与 Retina 缩放仍需实机确认。
- ✅ 单实例保护、文件日志、按需重绘和 `.app`/LaunchAgent/签名/公证脚本已实现；签名和公证仍需
  用户自己的 Developer ID 证书与 `notarytool` 凭据。

最小可用判定：**B1（左键）、B3（拖拽）、B5（转头）、B6（穿透）、B11（空闲 CPU）全部通过**，
macOS 才算真正可用。

## 0. 准备

```bash
# 只需一次
xcode-select --install

# 在仓库根目录
cargo test --workspace
cargo run -p petsona-app
```

- 用独立数据目录，避免污染真实数据：

```bash
PETSONA_HOME="$HOME/.petsona-mac-test" cargo run -p petsona-app
```

- 不设置 `PETSONA_HOME` 时，数据在 `~/Library/Application Support/Petsona/`。
- 日志同时写入 `logs/petsona.log` 和终端；Finder/LaunchAgent 启动时也可直接查看文件日志：

```bash
RUST_LOG=debug cargo run -p petsona-app
```

- 单实例锁位于数据目录的 `petsona.lock`；使用不同的 `PETSONA_HOME` 才会启动隔离实例。

## A. 基础回归（现在应该能通过）

| # | 操作 | 预期结果 | 状态 |
|---|---|---|---|
| A1 | 启动 | 宠物窗口出现、无边框、透明背景（不是黑底/白底）、置顶 | 待实测 |
| A2 | 看菜单栏 | 出现 Petsona 托盘图标 | 待实测 |
| A3 | 点击托盘图标 | 弹出 macOS 原生菜单：打开设置 / 显示隐藏宠物 / 退出 | ✅ 已验证 |
| A4 | 打开设置 | 能打开、滚动；缩放、穿透、状态协议端口、自动行走等控件可操作 | 待实测 |
| A5 | 切换宠物 | 本地库 + `~/.codex/pets` + `~/.unipet/pets` 都能列出；切换后动画和窗口/托盘图标更新 | 待实测 |
| A6 | 导入/导出 | 导入文件夹或 `.zip`（含拖放到窗口）；导出 Codex 上传格式；删除本地副本需确认 | 待实测 |
| A7 | 状态协议 POST | `waiting` / `failed` / `review` / `running` 能切换动画；带 `message` 时显示气泡；`ttlMs` 到期回 base | 待实测 |
| A8 | 状态协议 GET | `GET /health` 返回当前 pet/persona/state；`GET /pets` 返回 id 列表 | 待实测 |
| A9 | 重启持久化 | 当前宠物、人格、缩放等写入 `config.json`，重启后保持 | 待实测 |
| A10 | 托盘隐藏/显示/退出 | 隐藏后窗口消失，托盘可恢复；退出后进程真的结束 | 待实测 |
| A11 | DeepSeek keychain | 设置里保存 API key 后，Keychain 出现 Petsona 条目；重启后仍能读取（无 key 时回落固定问候） | 待实测 |

状态协议命令：

```bash
curl -s http://127.0.0.1:17872/health
curl -s http://127.0.0.1:17872/pets
curl -XPOST http://127.0.0.1:17872/state \
  -H 'content-type: application/json' \
  -d '{"source":"mac-verify","state":"waiting","message":"macOS 验证","ttlMs":10000}'
```

## B. 交互后端验收（代码已接入，需在 macOS 实机逐条打勾）

| # | 操作 | 预期结果 | 当前状态 |
|---|---|---|---|
| B1 | 左键单击宠物 | 挥手 + 气泡；320ms 内的第二次点击不应先触发单击 | ✅ 已验证 |
| B2 | 快速双击 | 跳一下；不先触发单击 | ✅ 已验证 |
| B3 | 按住拖动 | 宠物跟手移动；左右移动时播放 running-left/right；松手停下且不触发单击 | ✅ 已验证 |
| B4 | 右键 | macOS 原生菜单出现在光标位置；点菜单外或按 Esc 关闭 | ✅ 已验证 |
| B5 | 鼠标在宠物左右两侧移动 | row9/row10 转头动作各播放一次；0.9s 冷却；正前方死区不触发 | ✅ 已验证 |
| B6 | 开启 `click_through` | 透明像素点击落到桌面；不透明精灵像素仍能点击；关闭时整个窗口可交互 | ✅ 已验证 |
| B7 | 在 TextEdit/浏览器输入时点宠物 | 前台焦点不被打断，输入继续进入原应用 | ✅ 已验证 |
| B8 | 点击 / 右键 / 打开设置 | 宠物周围不出现任何边框闪烁 | 待实测（macOS 理论上无 Windows 那个问题） |
| B9 | 用状态协议发带 message 的 state | 气泡完整不被裁切；显示/消失时宠物不移动、窗口不闪烁 | 待实测 |
| B10 | 在副屏右键 | 菜单出现在光标所在显示器，且被夹在工作区内 | 待实测（当前无副屏条件） |
| B11 | 空闲时看 Activity Monitor | Petsona 空闲 CPU 接近 0–1%（允许偶发波动） | 待实测（已改为按需重绘） |
| B12 | 启动第二个实例 | 不出现第二只宠物；要么退出，要么唤起已有实例 | 待实测（已实现锁） |
| B13 | 注销再登录 / 重启 | 宠物自动出现 | 待实测（已提供 LaunchAgent 脚本） |

## C. 多显示器 / Spaces / 窗口系统

| # | 操作 | 预期结果 | 状态 |
|---|---|---|---|
| C1 | 把宠物拖到副屏（修好拖拽后） | 位置正确，缩放正确，不跳回主屏 | 待实测 |
| C2 | Retina + 非 Retina（或不同缩放）混用 | 精灵清晰、尺寸不跳、点击命中位置不漂 | 待实测 |
| C3 | 切换 Space / 进入全屏 App | 置顶行为符合预期；明确宠物是只在当前 Space 还是所有 Space | 待实测 |
| C4 | Mission Control / Stage Manager | 宠物不干扰窗口管理，托盘菜单仍可用 | 待实测 |
| C5 | 深色 / 浅色菜单栏 | 托盘图标在两种模式下都清晰可见 | 待实测 |
| C6 | 隐藏/显示后 | 窗口位置、层级、穿透状态保持一致 | 待实测 |

## D. 打包 / 发布

- [x] `cargo build --release` 编译通过；bundle 产物已完成结构检查
- [x] `scripts/package-macos.sh` 生成 `.app` bundle、Info.plist、bundle id、图标和 zip
- [x] `LSUIElement = true`，默认不显示 Dock 图标
- [x] `scripts/install-macos-launch-agent.sh` 安装/卸载 LaunchAgent
- [x] `scripts/sign-macos.sh` 提供签名和验证流程，并已用 ad-hoc 身份验证
- [x] `scripts/notarize-macos.sh` 提供 notarytool、stapler、spctl 流程
- [ ] 使用真实 Developer ID 证书和 Apple 凭据完成签名、公证，并在目标 Mac 实机验证
- [x] `.github/workflows/release-macos.yml` 在 tag / 手动触发时生成 macOS 架构包
- [x] 单实例保护（macOS 与 Windows 共用文件锁实现）

打包和开机启动命令：

```bash
./scripts/package-macos.sh
./scripts/install-macos-launch-agent.sh install dist/Petsona.app
./scripts/install-macos-launch-agent.sh uninstall
```

签名和公证需要用户配置证书身份与钥匙串 profile：

```bash
CODESIGN_IDENTITY="Developer ID Application: ..." ./scripts/sign-macos.sh
NOTARYTOOL_PROFILE="petsona-notary" ./scripts/notarize-macos.sh
```

## E. 记录模板

| 日期 | macOS 版本 | 芯片 | 构建方式 | 结论 | 备注 / 日志 |
|---|---|---|---|---|---|
|  |  |  | `cargo run -p petsona-app` |  |  |

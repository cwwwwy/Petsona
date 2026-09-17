# Petsona macOS 实机验收清单

CI 只能证明“能编译”，不能证明“能用”；**真实结论以本清单为准**。
最小可用判定：**B1（左键）、B3（拖拽）、B5（转头）、B6（穿透）、B11（空闲 CPU）全部通过**。

## 已知状态（2026-09-17）

- ✅ 已通过：A3、B1–B4、B6–B7；透明穿透、托盘菜单、设置聚焦、当前 Space 稳定。
- ⚠️ 待复测：B5 真实光标、B8 缩放视觉、B9 回复按钮 / 注视、B11 Activity Monitor（修复前 8–10%）。
- ⏳ 待实机：B10 多屏、B12/B13、C 节 Retina / Spaces、D 节真实签名 / 公证。
- ℹ️ 位置记忆存**物理像素**；macOS 工作区暂用 winit 整块显示器边界（Dock / 菜单栏未排除），
  多屏 / Retina 复验时重点看还原位置；重力落点同理，等 `NSScreen.visibleFrame` 后再精确。
- ℹ️ 开机自启走 `scripts/install-macos-launch-agent.sh`（设置里显示“不支持”），有意保留的差异。

## 0. 准备

```bash
xcode-select --install            # 只需一次
cargo test --workspace
PETSONA_HOME="$HOME/.petsona-mac-test" cargo run -p petsona-shell-macos
```

- 默认数据目录：`~/Library/Application Support/Petsona/`；日志写入 `logs/petsona.log` 和终端
  （`RUST_LOG=debug` 可调级别）。
- 单实例锁在数据目录的 `petsona.lock`；换 `PETSONA_HOME` 才能起隔离实例。

## A. 基础回归

| # | 操作 | 预期结果 | 状态 |
|---|---|---|---|
| A1 | 启动 | 宠物窗口出现、无边框、透明背景、置顶 | 待实测 |
| A2 | 看菜单栏 | 出现 Petsona 托盘图标 | 待实测 |
| A3 | 点击托盘图标 | macOS 原生菜单：打开设置 / 选择宠物（V2 有勾选）/ 显示隐藏 / 退出 | 代码完成；实机已通过一次，可复测 |
| A4 | 打开设置 | 能打开、滚动；缩放、穿透、协议端口、自动行走可操作 | 待实测 |
| A5 | 切换宠物 | 本地库 +「从 Codex 导入」可用；切换后动画与窗口 / 托盘图标更新 | 导入面板 2026-09-16 新增，待实测 |
| A6 | 导入 / 导出 | 原生文件面板或拖放导入；原生保存面板导出；删除需确认 | 待实测 |
| A7 | 状态协议 POST | `waiting`/`failed`/`review`/`running` 切换动画；message 显示气泡；TTL 回 base | 待实测 |
| A8 | 状态协议 GET | `/health`、`/pets` 返回正确 | 待实测 |
| A9 | 重启持久化 | 宠物、人格、缩放、位置写入 `config.json` 并保持 | 位置写入 smoke 通过；真实拖动重启待人工 |
| A10 | 托盘隐藏 / 显示 / 退出 | 隐藏后窗口消失、可恢复；退出后进程真的结束 | 待实测 |
| A11 | DeepSeek keychain | 保存后 Keychain 出现 Petsona 条目，重启仍可读；无 key 回落固定问候 | 待实测 |

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
| B4 | 右键 | 原生菜单出现在光标处；点外 / Esc 关闭 | 代码修复 + 自动化通过；真实触控板待复验 |
| B5 | 光标在宠物两侧 | row9/row10 保持最强方向帧；离开播放返回段；0.9s 冷却；死区不触发 | 状态机 smoke 通过；真实方向 / 返回动画待复验 |
| B6 | 开启 `click_through` | 透明像素点落到桌面；不透明像素仍可点；关闭后整窗可交互 | ✅ 已验证 |
| B7 | 在 TextEdit / 浏览器输入时点宠物 | 前台焦点不被打断 | ✅ 已验证 |
| B8 | 点击 / 右键 / 设置 / 改缩放 | 无边框闪；设置成为可输入前台窗口；`0.5 / 0.75 / 1.0 / 1.25 / 1.5 / 2.0` 缩放不闪 | Key Window 与几何回归自动通过；缩放视觉待复测 |
| B9 | 状态 message / 悬停气泡 / 双击 | 气泡完整；回复按钮可见；对话框可输入发送；输入时注视光标 | 对话流程 smoke 通过；视觉与真实注视待实测 |
| B10 | 在副屏右键 | 菜单出现在该屏并夹在工作区内 | 待实测（当前无副屏条件） |
| B11 | 空闲看 Activity Monitor | CPU 接近 0–1%（允许偶发波动） | 修复前 8–10%；低频采样 smoke 通过，待复测 |
| B12 | 启动第二个实例 | 不出现第二只宠物 | 待实测（锁已实现） |
| B13 | 注销 / 重启后登录 | 宠物自动出现 | 待实测（LaunchAgent 脚本已提供） |

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
- [x] `scripts/package-macos.sh` 生成 `.app`（Info.plist、bundle id、图标、zip）
- [x] `LSUIElement = true`，默认不显示 Dock 图标
- [x] `scripts/install-macos-launch-agent.sh` 安装 / 卸载 LaunchAgent
- [x] `scripts/sign-macos.sh`、`scripts/notarize-macos.sh` 流程就绪（ad-hoc 签名验证过）
- [x] `.github/workflows/release-macos.yml` tag / 手动触发生成架构包
- [ ] 真实 Developer ID 签名 + 公证 + 目标 Mac 实机验证

```bash
./scripts/package-macos.sh
./scripts/install-macos-launch-agent.sh install dist/Petsona.app
./scripts/install-macos-launch-agent.sh uninstall

CODESIGN_IDENTITY="Developer ID Application: ..." ./scripts/sign-macos.sh
NOTARYTOOL_PROFILE="petsona-notary" ./scripts/notarize-macos.sh
```

## E. 自动化验收

```bash
bash scripts/verify-macos-all.sh               # 门禁 + runtime smoke + 打包结构检查
bash scripts/verify-macos-all.sh --gates-only  # 只跑 fmt / clippy / test / release
```

入口脚本 = workspace 门禁（等价 `cargo check -p petsona-shell-macos`）+ `macos-smoke.sh` +
打包结构检查（可执行文件、图标、Bundle ID、`LSUIElement`、Retina 字段、LaunchAgent 模板）。
smoke 覆盖协议 / TTL、设置、Key Window、缩放锚点、位置保存、低频指针、idle 重绘、CPU 趋势、
V2 注视生命周期（`turning → holding → returning → idle`）。
**不覆盖**：缩放闪动观感、真实触控板 / 托盘点击、菜单外观、Activity Monitor 最终 CPU、
Retina / Spaces、多显示器。

V2 夹具默认从 `~/.codex/pets`、`~/.unipet/pets` 搜索并复制进临时 `PETSONA_HOME`
（应用只加载本地库）；也可以显式指定：

```bash
PETSONA_SMOKE_V2_PET_DIR="$HOME/.codex/pets/<pet>" bash scripts/macos-smoke.sh
```

> 2026-09-16 的平台后端搬移只在 Windows 上做过 fmt + 静态审阅（macOS 目标无法在 Windows 上
> 链接检查：`ring` 需要 macOS C 工具链），所以回 mac 后请先跑一次完整脚本。


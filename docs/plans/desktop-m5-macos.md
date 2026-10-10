# desktop-m5-macos：执行契约 v1.0

## 身份与授权

- 日期：2026-10-10；父任务 `desktop-shell-rust-ts` M5。
- 用户明确本会话顺序：“1. 设置页深度修复。2. M5阶段”，并授权“读取文档，了解项目，然后推进计划”。此前 M5 暂缓决定更新。
- 基线 HEAD `63dfc9b`；设置修复在本工作区进行，M5 产品代码尚未开始，不能将共用前端修改计为 macOS 功能完成。
- 本次范围：新 Tauri 壳在 macOS 的完整平台与原生浮层行为；共用既有设置、聊天、历史、人格和记忆 TS 页面。
- 非目标：恢复 Swift/C#/FFI，修改业务/协议/数据格式，恢复已下架设置项，签名公证/发布 workflow（留 M6）。
- 顺序：设置 S1–S4 实现与门禁、用户验收 → M5-A 基线与系统集成 → M5-B 宠物 → M5-C 气泡/Composer → M5-D 全量验收。
- 记录：`../execution/desktop-m5-macos.md`。

## 调查事实

1. `main.rs` 仅 `#[cfg(windows)]` 创建浮层；Mac 可创建内容窗和托盘，但宠物/气泡/Composer 无实现。
2. `dialog.rs` 非 Windows 路径返回未实现错误；`autostart.rs` 非 Windows 查询 false、设置报错。
3. `core::secrets::KeyringStore` 已支持 macOS；根 `Cargo.toml` keyring 已启用 `apple-native`。不得重复造凭据库。
4. `settings.rs` 打开目录/链接已用 macOS `open`；TS 可复用，需实际验证。
5. 现有 lockfile 已含 objc2 0.6.5 / objc2-app-kit、foundation、core-graphics 0.3.2；AppKit 绑定包含 NSGlassEffectView。
6. 2026-10-10 Mac arm64 基线 `cargo check --locked` 失败：Tauri `generate_context!` 要求 `src-tauri/icons/icon.png`，工作区只有 ICO。
7. 最低 macOS 26 的既有决定继续有效；当前机器 macOS 27.0.1，不能替代 macOS 26 或 Intel 实机证据。

## REQ 与完成标准

| REQ | 要求 | 完成标准 |
|---|---|---|
| M5-01 | Mac 构建、图标和最低版本 | arm64 debug/release 构建通过，图标正确，最低版本配置 26；不改 Windows 图标 |
| M5-02 | 菜单栏与内容窗生命周期 | 设置/聊天显示时 Dock 可见，最后一个内容窗关闭恢复菜单栏模式；打开可聚焦，关闭只隐藏；无焦点循环 |
| M5-03 | 原生文件对话框 | ZIP/文件夹、人格/记忆 JSON、TXT/JSON 来源、导出全可用；取消无副作用；保持既有 IPC |
| M5-04 | LaunchAgent | 系统页查询/开启/关闭一致；plist 合法、路径带空格正确；真实登录人工确认 |
| M5-05 | Keychain | 已有供应商隔离语义；保存/读取/删除/重启；不把 Key 写入设置/日志/快照；自动检查用假凭据，真实授权人工 |
| M5-06 | 原生宠物与动画 | 非激活透明 NSPanel，无影子/白底，V1/V2 与 webp 正确；runtime 192×208 cell、帧变化才重绘；帧序持续推进 |
| M5-07 | 点击、拖动、注视与几何 | 透明像素穿透、idle 并集命中；单击/双击/拖动命令；16方向注视；工作区夹取、尺寸锚点、物理位置持久化；Retina/多屏/Spaces 人工 |
| M5-08 | 气泡与输入入口 | 原生玻璃气泡、150ms 淡入、剩余进度、悬停暂停/续跑、点击打开；宠物滚轮下开/上关；不恢复 Windows 已删除的编辑条 |
| M5-09 | 原生 Composer | AppKit NSTextView 原生 IME；玻璃、无标题栏、紧凑自适应多行、圆形发送；Enter/Shift+Enter/Esc草稿；跟随/侧挂、输入光标注视 |
| M5-10 | 故障、退出与单实例 | 同目录第二实例提示后退出；首实例继续；空库首启设置；退出释放协议/托盘/全部窗口，无后台残留 |
| M5-11 | 共用内容界面与共享回归 | 六页、聊天/历史、人格来源与记忆审阅在 WKWebView 实际可用；Windows 受影响项重新验证 |

## 逐文件变更

| 路径 | 动作 | 具体职责 | REQ |
|---|---|---|---|
| `apps/desktop/src-tauri/Cargo.toml`、`Cargo.lock` | 修改 | Mac target 声明已在锁中存在的 objc2/AppKit/Foundation 等直接依赖；锁版本不漂移 | 01、06–09 |
| `apps/desktop/src-tauri/tauri.conf.json` | 修改 | macOS 最低版本与通用图标；不启用发布打包 | 01 |
| `apps/desktop/src-tauri/icons/icon.png` | 新增 | 从现有品牌资产导出用于 Tauri Mac 上下文的 PNG | 01 |
| `apps/desktop/src-tauri/src/main.rs` | 修改 | Mac 主线程初始化/更新浮层、Dock策略、主题、退出、测试参数、菜单栏图标；保留 Windows cfg 分支 | 02、06–10 |
| `apps/desktop/src-tauri/src/macos/{mod,overlay,geometry,composer}.rs` | 新增 | 平台入口、NSPanel/绘制/命中、纯几何转换、NSTextView/原生玻璃；可按职责局部拆分 | 06–09 |
| `apps/desktop/src-tauri/src/dialog.rs` | 修改 | Mac 在主线程使用 NSOpenPanel/NSSavePanel，复用既有命令名；不调用 AppleScript | 03 |
| `apps/desktop/src-tauri/src/autostart.rs` | 修改 | LaunchAgent路径/标签隔离、plist原子写入/删除与查询；测试不得动真实自启 | 04 |
| `scripts/desktop-build-macos.sh` | 新增 | 前端+Mac debug/release 编译，保留目标、架构、退出码 | 01 |
| `scripts/desktop-smoke-macos.py` | 新增 | 标准库隔离目录/协议/单实例/退出/空库，原生诊断日志与截图证据；不注入输入 | 06、10 |
| `docs/DESKTOP_VERIFICATION.md`、`README.md`、`docs/PLATFORM_ARCHITECTURE.md` | 修改 | 当前 Mac 构建入口、线程边界、人工验收与限制 | 全部 |
| `docs/plans/desktop-m5-macos.md`、`docs/execution/desktop-m5-macos.md` | 新增/维护 | 逐REQ与证据，不覆盖失败/SKIP | 全部 |

## 实现约束与依赖

- AppKit 只能在 Tauri 主线程操作；runtime worker 与纯图集解码可后台。不得把 Windows 独立浮层线程/消息泵照搬到 AppKit。
- Rust 持有 NSPanel/视图/事件；TS 不绘制宠物、气泡或 Composer。跨线程只入队命令与读不可变投影。
- `PhysicalRect` 是壳内唯一几何单位；NSScreen/AppKit 点与 bottom-left 坐标只在边界转换；多屏/负原点/不同 backing scale 有纯计算测试。
- 不修改 config/startPosition 与现有 macOS sidecar 兼容语义；发现互斥坐标约定时记录 CR 暂停相应几何项。
- 玻璃使用已绑定的 NSGlassEffectView（macOS 26+），只用于气泡和 Composer；宠物透明图集不加模糊。
- 优先复用已锁依赖。若需锁中没有的新库、改变协议或数据格式，记录 CR 并交回用户。
- 自动实例必须隔离 `PETSONA_HOME`、协议端口（17943/17944）、自启标签/目录与凭据。现有 startup 会调用 OS 凭据探测；在隔离通路完成前不启动默认 runtime。
- LaunchAgent 自动测试写到隔离目录，仅 lint/read-back；真实 `~/Library/LaunchAgents`、launchctl与真实Keychain授权由人工专项执行。
- 不移动鼠标、不注入键盘/窗口消息。点击、拖动、滚轮、悬停、注视、Tab/IME/菜单全部人工验收。
- Git只读；所有未提交改动分组摘要与提交命令作为交付内容。

## 验收矩阵

| ID | REQ | 环境/命令或步骤 | 必需证据 |
|---|---|---|---|
| T-M5-01 | 01 | 现有 `pnpm --dir apps/desktop build`、`cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --locked` | cwd、arm64、退出码、产物路径；check 不等于运行通过 |
| T-M5-02 | 全部 | 实施后 `scripts/desktop-build-macos.sh` debug/release；桌壳 fmt/clippy/test；根 workspace 门禁 | 每条命令记录，失败保留；放行回环重跑环境性失败 |
| T-M5-03 | 04、07 | 几何单测/隔离 plist 序列化与 `plutil -lint` | Retina/负原点/边缘夹取、含空格路径、写删往返 |
| T-M5-04 | 06、10 | 实施后 `python3 scripts/desktop-smoke-macos.py` | 隔离夹具启动、动画诊断、协议TTL/clear/400、第二实例、退出端口释放、空库 |
| H-M5-01 | 02、03、11 | Mac 六页/原生文件对话框与菜单栏/设置/聊天开合 | 取消、聚焦、Dock恢复、自动保存、资料学习与候选审阅 |
| H-M5-02 | 06–09 | 本机浅深背景、点击穿透、拖动、滚轮、注视、气泡、原生输入/中文IME | 逐项回复，通过/失败与截图日志 |
| H-M5-03 | 04、05 | 专项真实登录自启与Keychain保存/读取/删除 | 人工明确通过；未做保持待验收 |
| H-M5-04 | 07 | Retina、多屏混合DPI、负原点、Spaces、睡眠恢复 | 硬件不可用写SKIP/待验收，不算全部完成 |
| H-M5-05 | 全部 | macOS 26、Intel | 环境缺失保持待验收；不伪称当前27/arm64覆盖 |
| T-WIN-01 | 11 | Windows既有构建、无输入检查、受影响设置页人工复测 | Mac证据不能替代；旧冒烟含注入输入段，先按当前硬规则核对/隔离禁用后运行 |

## 暂停与完成

- 设置页仍需 Windows 构建/无输入回归、人工验收；M5 调查与文档可独立推进，产品实施按用户顺序在设置收口后开始。
- 任一必要验收不可用，记录环境/影响并保留待验状态；不修改标准使现状合规。
- M5-A/B/C 各批交付人工清单；M5-D 逐REQ闭合才能宣布 M5 完成，之后才回到 M6。

## 修订

| 版本 | 用户依据 | 变更 |
|---|---|---|
| v1.0 | 2026-10-10 两步顺序与推进授权 | 恢复M5，基于新壳调查细化范围；实施未开始 |
| v1.1 | 2026-10-10 用户「先准备资产，随后切 Mac」 | 无 REQ/验收变更 | Windows 侧完成 `icon.png`（1024）/`icon.icns`/`bundle.macOS.minimumSystemVersion=26.0` 准备，Windows 构建回归通过；待 Mac 复核编译 |

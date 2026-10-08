# Petsona 项目须知（给 AI 协作者）

## 项目速览

Petsona 是 Windows / macOS 桌宠与 AI 伴侣。项目正按 `desktop-shell-rust-ts` 计划重建为两条技术线：

- **Rust**：`petsona-core` / `petsona-runtime`（业务、动画、记忆、人格、协议，已稳定）+ 桌面壳的平台层
  （托盘、窗口、几何、输入、凭据、自启）与**浮层渲染**（宠物 / 气泡 / 编辑条 / Composer，原生窗口绘制）。
- **TypeScript**：全部内容界面（设置六页、聊天、历史、人格、宠物库），一份代码两端复用。

**WebView 不渲染宠物浮层。** 旧 C#/WinUI 与 Swift/AppKit 前端、C ABI（`petsona-ffi`）已于 2026-10-08 删除；
完整快照为 git `6bca241`，需要对照时只读 `git show 6bca241:<path>` 取回，不恢复死代码到工作树。

平台现状：Windows 为主线（在新壳上重建）；macOS 在 M5 平齐。产品入口 `apps/desktop` 处于 M0 建设期。

## 目录

| 路径 | 内容 |
|---|---|
| `crates/petsona-core/` | 宠物格式与动画引擎、人格、记忆、DeepSeek、状态协议 |
| `crates/petsona-runtime/` | 配置与会话、宠物库、实例锁、日志、问候、网络任务、流式聊天、记忆学习、人格生成 |
| `apps/desktop/` | 新桌面壳（Tauri 2）：`src-tauri/`（Rust 平台层 + 浮层）+ `src/`（TS 内容界面）——建设中 |
| `docs/` | 架构、验收清单、`plans/` 与 `execution/` 跨对话工作流文档 |

## 常用命令

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace          # core + runtime；状态协议测试需要绑定本地回环端口
```

- `apps/desktop` 的构建与验收入口在 M0 建立后写入 `docs/DESKTOP_VERIFICATION.md` 与 `README.md`。
- 测试必须隔离 `PETSONA_HOME`、端口、自启值名与凭据，不得启动默认用户实例。
- 受限沙箱禁止绑定回环端口，协议测试会以环境性错误失败；应在放行网络的环境复跑并保留对照记录。

## 协作偏好（硬规则）

- 用中文交流。
- **Git 只做只读查询，不代替用户操作**：`status` / `log` / `diff` / `show` / `remote -v` / `branch -vv` 可以；
  `add` / `commit` / `push` / 建删分支 / tag / 改 remote 一律给命令让用户自己跑，除非用户明确说“你来操作”。
- 计划契约与执行证据分别写入 `docs/plans/<任务名>.md` 与 `docs/execution/<任务名>.md`（模板已保留）；
  长期决策与规则在获准修改文档时维护到本文件。
- 新会话先读：`AGENTS.md`、`docs/PLATFORM_ARCHITECTURE.md`、`docs/DESKTOP_VERIFICATION.md`、
  `docs/plans/desktop-shell-rust-ts.md`、`docs/execution/desktop-shell-rust-ts.md`、git log。
- 尽量不新增第三方依赖：`petsona-core` / `petsona-runtime` 优先标准库与现有依赖；`apps/desktop` 已获准使用
  Tauri 2 + Vite/React/TS 生态，版本锁定并提交 lockfile。
- 涉及窗口 / 托盘 / 菜单 / 浮层的改动，交付时写清“需要用户实机确认什么”。
- **提交总结是交付的一部分**：每轮修改后给出**所有未提交改动**的分组摘要与可直接执行的
  `git add` / `git commit` 命令；AI 不执行提交。

## 跨对话工作流（规划 → 执行 → 审查）

- 当前任务线：`desktop-shell-rust-ts`（P0/M0/M1 已提交；M2-A 真实图集渲染/缩放/位置记忆完成待人工；M2-B 注视/气泡/编辑条/Composer 待做；macOS 可行性待补）。
- **规划**：只读调查，明确目标/非目标、逐文件增改删、约束、REQ 编号、依赖、验收矩阵、命令与完成条件；
  授权落盘后才写指定文档，不写产品代码。
- **执行**：先复述关键目标与验收标准，再按计划实施；可作计划内局部实现选择，不得自行缩减功能、
  将完整交付改成骨架、跳过验收或改变架构边界。
- **暂停**：基线重叠冲突、目标互斥、必须改变契约/依赖/数据格式/平台范围，或无法满足必要验收时，
  说明文件证据、影响、建议与所需决定，暂停受影响工作。
- **计划变更**：执行者只能在执行记录中提出偏差/变更请求，不能修改目标或验收来使现状“合规”。
- **证据**：每个 REQ 分开记录实现、自动测试、人工验收状态；命令须记录目标程序、架构、退出码、
  执行时工作区与结果位置；保留失败与 SKIP；旧入口/历史结果不能替代当前验收。
- **完成**：所有必需项通过、有证据、无未解决审查项才可宣布完成；人工未做时写“待验收”，不降低标准。

## 当前状态（2026-10-08）

- P0 `1e3b5c1`、M0 `534d0f8` 已提交；当前未提交为 M1 批次（runtime 接入 + 冒烟扩展）。
- `crates/petsona-core` / `petsona-runtime` 门禁 **114/114**（core 86 / runtime 28）。
- M1：`apps/desktop` 直接依赖 `petsona-runtime`（无 FFI）——数据目录/`PETSONA_HOME`、日志、`petsona.lock` 单实例、
  `stateServer.port` 协议（/health /pets /state + TTL/400/clear）、托盘（设置… / 显示-隐藏 / 退出）、
  空库首启自动开设置、退出释放端口，均由 runtime 驱动；浮层可见性 = `ready && has_pet && pet_visible`。
  `scripts/desktop-smoke.ps1` 完整冒烟 EXIT=0；启动（含 runtime 与宠物加载）**0.75–0.88s**。
- M2-A（未提交）：浮层改为 runtime 驱动——真实图集渲染（boba 已实机验证）、33ms 轮询动画、scale×DPI（钳制 ≥1）与 bottom-centre 锚点、
  拖动结束写 `startPosition` 并在启动时恢复、全程 `rcWork` 夹取；`desktop-smoke.ps1` EXIT=0（几何 288×312=192×208×1.5、动画 2 帧、位置往返 dx=0）。
- M2-B 待做：注视采样与迟滞、气泡（淡入/进度/悬停暂停）、编辑条、Composer、故障提示气泡。
- 构建入口：`scripts/desktop-build-windows.ps1`（Windows）；冒烟 `scripts/desktop-smoke.ps1`（`-SkipMouseChecks` 供鼠标忙时）；
  截图 `scripts/desktop-shot.ps1`。
- 旧世界验收知识已迁移：行为矩阵在 `docs/DESKTOP_VERIFICATION.md`，产品决策在计划文档「继承的产品决策」。
- 发布目标：新壳首个正式版 `windows-v0.1.0`（旧 `0.1.0-rc.1` 未发布，作废）。

## 架构约定

- Rust workspace 只含 `petsona-core` / `petsona-runtime`；`apps/desktop` 是独立的 Tauri 壳工程
  （暂不加入根 workspace，避免 Linux 门禁依赖 WebKit；M6 再评估）。
- Rust 拥有全部业务状态与窗口；TypeScript 只做内容界面与展示逻辑，不持有业务真相。
- 跨边界只传 JSON 投影与命令；不传 Rust 引用/容器/分配器所有权；不再有 C ABI。
- `PhysicalRect`（物理像素）是几何唯一单位；逻辑点只在前端边界换算。
- 浮层窗口（宠物 / 气泡 / 编辑条 / Composer）必须原生渲染；设置 / 聊天 / 历史 / 人格 / 宠物库用 WebView。
- 数据目录、`config.json`、宠物格式、协议语义保持兼容（见「关键事实速查」与验收清单附录）。
- 发布：`windows-v*` / `macos-v*` 独立 tag 与 workflow（M6 重建）。

## 关键事实速查

- 数据目录：`%APPDATA%\Petsona` / `~/Library/Application Support/Petsona`，`PETSONA_HOME` 可覆盖。
- 宠物库：只加载本地 `...\Petsona\pets`；`~/.codex/pets` 仅作为「从 Codex 导入」来源；
  **无内置宠物**，本地库为空时启动直接打开设置窗口；测试用自绘夹具 `crates/petsona-core/testdata/v2-test-pet`。
- 渲染格：runtime 一律按 **192×208**（1536×2288 ÷ 8×11）报告 V2 cell，**不读取宠物包内的 `grid.cellWidth/Height`**；
  测试夹具虽声明 64×64 也按 192×208 渲染（旧 C# 行为一致）。
- 配置：`config.json`；`window.startPosition` 为物理像素；缩放 0.5–2.0、吸附 7 档；
  协议端口键为 **`stateServer.port`**（camelCase，默认 17872）——写错键会被 serde 默认值静默忽略。
- 状态协议：`127.0.0.1:17872`，`POST /state`、`GET /health`、`GET /pets`；验收实例用独立端口（如 17873）。
- 开机自启（Windows）：`HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 的 `Petsona` 值；macOS 用 LaunchAgent（M5）。
- 日志：数据目录 `logs/petsona.log`；单实例锁：`petsona.lock`。
- 启动耗时基线（旧实现 E-W15d，M0 对照）：热 ~0.55s / 冷 ~3.2s。
- 协议 source 语义、TTL、`action:"clear"` 与手工命令：见 `docs/DESKTOP_VERIFICATION.md` 附录。

## 坑与教训（别再重新推导）

- **拥有窗口的线程必须抽消息**：跨线程同步消息（如 `GetWindowTextW`）会永久阻塞；窗口查询必须确认所属线程与消息循环。
- **Win11 会给顶层窗口画系统边框 / 圆角**：透明窗口需 `DWMWA_BORDER_COLOR = DWMWA_COLOR_NONE` +
  `DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_DONOTROUND`（Rust 侧沿用；旧系统拒绝该属性，失败无害）。
- **V2 注视是方向姿势表**：look-row-9/10 每帧是目标姿势；跨行先经旧行边缘姿势再进入目标；
  触发区为椭圆（半尺寸 + 短边 25% 留白），退出留白 35%，姿势步进 16ms；离开触发区回中性帧。
- **转向姿态会让命中像素变透明**：像素命中要回退 idle 全帧并集掩码，否则光标在宠物上时点不到。
- **位置记忆必须物理像素**：工作区夹取用显示器 `rcWork`（任务栏为边界）；混合 DPI 与多屏仍需实机验证。
- **HKCU Run 在受限会话会 ACCESS_DENIED**：该场景测试应明确 SKIP，不误报失败。
- **不要每帧重复发窗口尺寸 / 层级**：用缓存比较，避免 `SetWindowPos` 造成的闪烁。
- **窗口类不设光标会“冻住”光标形状**：注册窗口类时设箭头光标并处理 `WM_SETCURSOR`（托盘 owner 窗口同样）。
- **`HTTRANSPARENT` 只在同线程窗口可靠转发**：跨进程需要 `WS_EX_TRANSPARENT`，会引入抖动——
  “像素级穿透”与“零闪烁”需要权衡；旧实现的取舍落在 `git show 6bca241:apps/windows/...` 可查。
- **逐像素浮层实现参照**：`WS_EX_LAYERED` + `UpdateLayeredWindow`（旧 `LayeredPresenter.cs`），新壳 Rust 移植以此为参照。
- **协议 socket 非阻塞继承**：`accept()` 后要显式恢复阻塞，否则偶发空响应；core 已有回归测试。
- **窗口过程可被同步重入**：`SetWindowLongPtrW`（样式变化）、`SetWindowPos` 等会同步投递消息回到窗口过程；
  持有状态的借用时重入会 panic，而 panic 跨 `extern "system"` 会直接终止进程。窗口过程内用 `try_borrow` 防御，
  重入消息交 `DefWindowProcW`。
- **PowerShell ↔ P/Invoke 的 `$null` 陷阱**：传给字符串参数时 `$null` 会被编组成**空串**（如 `FindWindowW` 变成找空标题窗口）；
  要传 `[NullString]::Value`。
- **合成鼠标测试会被真实鼠标污染**：`SetCursorPos`/`mouse_event` 期间必须保持鼠标空闲；真实拖动轨迹是 1px 级高频移动，
  可在日志中与合成测试（大步长）区分。拖动测试应取**离窗口中心最近的可点击像素并允许重试**——
  边缘像素会被轻微漂移变成穿透点击。
- **WSL 起来的 PowerShell 会污染 `PATHEXT`**：新脚本开头恢复 Windows 默认值，否则命令静默失败。
- **PowerShell 5.1** 含中文的 `.ps1` 必须 UTF-8 BOM；`Compress-Archive` 反斜杠问题用 `ZipFile::CreateFromDirectory` 规避。
- **macOS 代码必须在 Mac 上验证**：objc2/AppKit 改动在 Windows/Linux 只能审查，不能算通过。
- **webp 是一等格式**：宠物图集优先 `webp`；PNG/JPEG 兼容路径保留。

## Git 工作流

```bash
git fetch --prune
git pull --ff-only
# ...改代码...
git add -A
git commit -m "..."
git push
```

直接在 `main` 上开发；试验性改动可开 `codex/win-*` / `codex/mac-*` 短期分支，合并后删除。
全局配置已设好：`pull.ff=only`、`push.autoSetupRemote=true`、`fetch.prune=true`、`core.longpaths=true`。

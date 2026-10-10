# desktop-m5-macos：执行与审查记录

## 接手（2026-10-10）

- 契约：`../plans/desktop-m5-macos.md` v1.0；用户更新 M5 暂缓决定，顺序为设置深度修复后 M5。
- HEAD `63dfc9b`；接手时干净，当前 dirty 为设置 S1–S4 与文档。
- 当前阶段：只读调查、环境核对、基线编译；M5 产品实现未开始。
- 目标：Rust原生macOS浮层/平台系统集成，TS六页与聊天共用；不恢复旧前端/FFI。

## REQ 状态

| REQ | 实现 | 自动证据 | 人工验收 | 下一步 |
|---|---|---|---|---|
| M5-01 | 图标资产已就绪（Windows 侧，2026-10-10）；最低版本已配 26.0；Mac 侧编译待复核 | E-M5-03…06 | 未做 | Mac 上 `cargo check --locked` 复核 |
| M5-02 | 现有Tauri托盘/内容窗；Dock策略缺失 | 未运行 | 未做 | 主线程策略 |
| M5-03 | 非Windows对话框占位 | 未跑 | 未做 | NSOpenPanel/NSSavePanel |
| M5-04 | 非Windows自启占位 | 未跑 | 未做 | 隔离LaunchAgent |
| M5-05 | 既有keyring apple-native；未实机验证 | 代码调查 | 未做 | 先隔离测试凭据 |
| M5-06…09 | 未实现；Windows overlay未在Mac接入 | 代码调查 | 未做 | 主线程NSPanel与NSTextView |
| M5-10 | runtime协议/锁已有；Mac故障退出缺浮层路径 | 未跑 | 未做 | 接入原生故障/退出 |
| M5-11 | 共用TS修复中 | 前端build通过 | 未做 | WKWebView与Windows复测 |

## 证据

| ID | cwd/目标/架构 | 命令 | 退出码与结果 |
|---|---|---|---|
| E-M5-01 | `/Users/book/Desktop/Petsona` / Mac arm64 | sw_vers / uname -m / xcode-select -p / cargo --version / rustc --version | 0；macOS27.0.1、Xcode、Rust1.98.0 |
| E-M5-02a | 同上 / 新壳 arm64 | `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --offline` | 101；本机缓存只含Tauri2.11.5，与锁2.12.1不符；环境性失败保留 |
| E-M5-02b | 同上 / 新壳 arm64 / 放行联网 | `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --locked` | 101；依赖下载编译后 `generate_context!` 缺 `src-tauri/icons/icon.png`；非窗口运行证据 |

## 审查与接续

- REV-M5-01：基线不能编译；Tauri Mac上下文缺PNG图标。纳入M5-01，未修复。
- REV-M5-02：macOS没有原生浮层时，watcher在锁冲突后仍等待浮层退出，可能残留第二实例。纳入M5-10，未修复。
- REV-M5-03：现有Windows冒烟有WM_CHAR/WM_KEYDOWN等输入注入，不能直接作为本次“无输入”验证；保留历史证据，后续核对并隔离禁用。
- 当前Mac未启动用户实例，未访问真实Keychain、LaunchAgent或用户宠物库。
- 下一步：设置页自动/Windows人工验收 → M5-A；M5不得宣称完成。

## Windows 侧资产准备（2026-10-10）

用户指示「先准备资产，准备好后切 Mac」。本轮只做平台无关准备，**macOS 产品代码仍未开始**。

| ID | 内容 | 命令/方法 | 结果 |
|---|---|---|---|
| E-M5-03 | 导出 macOS 上下文 PNG | 从 `packaging/macos/Petsona.icns` 提取内嵌 PNG（1024×1024）写入 `apps/desktop/src-tauri/icons/icon.png` | 1024×1024 / 106 438 bytes；PNG 头校验通过 |
| E-M5-04 | 复制 macOS 打包图标 | `packaging/macos/Petsona.icns` → `apps/desktop/src-tauri/icons/icon.icns` | 12 个 chunk（1024/512/256/128/64/32 + ARGB mask），declared=actual=175 960 bytes |
| E-M5-05 | 更新 `apps/desktop/src-tauri/tauri.conf.json` | `bundle.icon = [icons/icon.icns, icons/icon.ico, icons/icon.png]`；`bundle.macOS.minimumSystemVersion = "26.0"` | `python3 -m json.tool` 通过 |
| E-M5-06 | Windows 侧回归（防止图标列表破坏现有构建） | `scripts/desktop-build-windows.ps1` + `scripts/desktop-settings-smoke.ps1`（隔离目录/端口 17899，无鼠标输入） | 构建 exit 0；冒烟 **PASS**（前台 True / 关闭只隐藏 / 重显 53 色） |

基线阻塞 **REV-M5-01（缺 `icons/icon.png`）已解除**，待 Mac 复核。

### 切 Mac 后的第一步

1. `git pull --ff-only` 同步本批资产与文档；
2. `pnpm --dir apps/desktop build` —— `generate_context!` 需要 `../dist` 已存在（Mac 首次需 `pnpm --dir apps/desktop install --frozen-lockfile`）；
3. `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --locked` —— 预期 `icons/icon.png` 报错消失，记录新的编译状态与新增报错；
4. 通过后按计划进入 **M5-A**：Dock/菜单栏策略、NSOpenPanel/NSSavePanel（`dialog.rs`）、LaunchAgent（`autostart.rs`）、Keychain 验证；
   记住约束：AppKit 窗口必须由 Tauri 主线程管理，不照搬 Windows 浮层线程；macOS 26 与 Intel 证据仍需真机（当前机器 27.0.1/arm64 不能替代）。

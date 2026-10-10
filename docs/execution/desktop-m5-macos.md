# desktop-m5-macos：执行与审查记录

## 接手（2026-10-10）

- 契约：`../plans/desktop-m5-macos.md` v1.0；用户更新 M5 暂缓决定，顺序为设置深度修复后 M5。
- HEAD `63dfc9b`；接手时干净，当前 dirty 为设置 S1–S4 与文档。
- 当前阶段：只读调查、环境核对、基线编译；M5 产品实现未开始。
- 目标：Rust原生macOS浮层/平台系统集成，TS六页与聊天共用；不恢复旧前端/FFI。

## REQ 状态

| REQ | 实现 | 自动证据 | 人工验收 | 下一步 |
|---|---|---|---|---|
| M5-01 | 未实现；发现缺icon.png | E-M5-02失败 | 未做 | 基线图标/最低版本修复 |
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

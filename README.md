# Petsona

[简体中文](README.md) | [English](README.en.md)

Windows / macOS 桌宠与 AI 伴侣。宠物在桌面上陪伴你，支持聊天、查看历史、编辑角色设定，以及管理关于你、共同经历和约定的记忆。

当前版本为 0.1.0 发布准备。桌面壳、功能验收与 M6-A 清理已通过；安装包、签名、公证与安装升级验收尚未完成。项目 Git 历史已重建为单一初始提交。

## 使用与数据

首次启动时，本地宠物库为空会显示设置。通过“导入”选择宠物文件夹或 ZIP，也可从 Codex 宠物库导入；应用没有内置宠物。角色由名称和一段可见设定构成，可从资料生成并试聊；长期记忆可查看来源、纠正或忘记。聊天历史与长期记忆分别管理。

- Windows 数据：%APPDATA%\Petsona；macOS 数据：~/Library/Application Support/Petsona（最低系统版本 26）。
- 日志位于数据目录的 logs/petsona.log 和 logs/shell.log，各最多 4 MiB 并保留一份轮转备份。
- 模型密钥存入系统凭据库，不写入配置或日志。登录自启可在系统页管理。
- 状态协议默认监听 127.0.0.1:17872，提供 POST /state、GET /health 和 GET /pets。

## 架构

petsona-core 和 petsona-runtime 负责业务状态、宠物动画、角色、记忆、会话与协议。apps/desktop/src-tauri 管理平台窗口、托盘、自启、凭据和原生浮层；apps/desktop/src 承载设置、聊天等内容页。

宠物、气泡和输入框由 Rust 原生窗口绘制；WebView 只承载内容界面。Rust 保有业务真相，前端通过 JSON 投影和命令通信。几何统一使用物理像素，数据格式与用户数据目录保持兼容。项目只维护 Rust/TypeScript 桌面壳，不包含旧 C#/Swift 前端或 C ABI。

## 开发与构建

Rust 工具链固定为 1.98.0。前端需要 Node.js 和 pnpm；Mac 构建需要 Xcode；Windows 构建脚本使用固定 MSVC 工具链和 Visual Studio Build Tools。

~~~sh
pnpm --dir apps/desktop install --frozen-lockfile
pnpm --dir apps/desktop build
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
~~~

状态协议单测需要绑定本地回环端口；受限环境拒绝绑定时应在允许本地网络的环境重跑。测试必须使用隔离的数据目录和端口。

## 桌面构建与回归

构建脚本默认生成本机验收版，显式带有验收专用检查；普通版使用 formal 模式，不包含自动退出、截图与原生验收入口。两种构建使用不同输出目录。

macOS 构建会先生成前端：

~~~sh
bash scripts/desktop-build-macos.sh debug
python3 scripts/desktop-verify-macos.py
bash scripts/desktop-build-macos.sh release formal
~~~

回归程序仅操作自己创建的进程、窗口和数据，不移动鼠标，也不注入键盘或滚轮输入。手动点击、拖动、输入法、托盘和视觉效果由用户直接验收。

Windows/WSL 先生成前端，再按需构建并回归一个配置：

~~~sh
petsona_repo_win="$(wslpath -w "$PWD")"
pnpm --dir apps/desktop build
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$petsona_repo_win\scripts\desktop-build-windows.ps1" -RepoRoot "$petsona_repo_win" -Profile release
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$petsona_repo_win\scripts\desktop-verify-windows.ps1" -RepoRoot "$petsona_repo_win" -Profile release
~~~

Windows 构建产物位于 %USERPROFILE%\petsona-build\desktop-target；Mac.app 位于 .scratch/macos/{debug,release}/Petsona.app。Mac 本地程序未签名，不是正式安装包。

## 发布前还需验收

M6-B 将制作 Windows 与 macOS 安装包并准备发布流程。候选包生成后，再集中验收首次安装、升级时保留宠物/角色/记忆/设置、登录自启、卸载后的数据保留、Windows 运行时处理和 macOS 首次打开。

## 开发文档

长期开发计划与阶段执行结果分别维护在[开发计划](docs/DEVELOPMENT_PLAN.md)和[开发执行记录](docs/DEVELOPMENT_EXECUTION.md)。

## License

MIT

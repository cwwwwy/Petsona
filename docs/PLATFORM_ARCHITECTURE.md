# Petsona 平台架构

当前产品入口均为原生前端。旧 egui UI、`PlatformHost` 与两个旧 shell 已删除，历史设计可从 Git 查询。
现行功能状态见 [FEATURE_PARITY.md](FEATURE_PARITY.md)，跨对话契约与证据分别放在 `plans/` 与 `execution/`。

## 分层与职责

```text
petsona-core + petsona-runtime → petsona-ffi (C ABI 3)
                                  ├── apps/macos (SwiftUI + AppKit)
                                  └── apps/windows (C# + WinUI 3 + Win32)
```

| 层 | 负责 |
|---|---|
| `petsona-core` | 宠物格式、动画与注视规则、状态优先级、人格/记忆数据模型、兼容模型请求 |
| `petsona-runtime` | 配置与会话、宠物库、文件存储、实例锁、日志、协议、网络任务、流式聊天、记忆学习、人格生成与取消 |
| `petsona-ffi` | 命令入队、快照与 JSON 文本投影、渲染数据、错误与句柄生命周期 |
| 原生前端 | UI 主线程、窗口与屏幕坐标换算、原生文本输入、菜单、渲染、Keychain/凭据与登录自启 |

`contracts/petsona.h` 与 `contracts/ABI.md` 定义跨语言边界。禁止跨 ABI 传递 Rust 引用、容器或分配器所有权；保留 ABI 3 结构布局与已有枚举值，新业务通过追加命令和 JSON 投影表达。

几何持久化使用物理像素；AppKit 屏幕点与像素只在前端边界换算。macOS 使用 `NSScreen.visibleFrame` 排除菜单栏与 Dock；混合 DPI、屏幕热拔插和 Spaces 仍需真实桌面验收。

## 产品与数据

macOS 最低支持 26，使用 SwiftUI 原生设置与 AppKit 宠物/浮层窗口。宠物、气泡和入口不抢焦点；输入框可以成为 Key Window，设置拥有独立主窗口。浮层采用 macOS 26 `NSGlassEffectView`。

宠物只来自 Petsona 本地库，无内置宠物；Codex 目录只作为显式导入来源。聊天历史与长期记忆按宠物隔离并分别控制；人格更新保留稳定身份和记忆。模型来源材料按数据处理，输出验证通过后才能应用。

macOS 用户数据默认位于 `~/Library/Application Support/Petsona`，Windows 位于 `%APPDATA%\Petsona`；测试在应用初始化前设置 `PETSONA_HOME`。凭据不写进配置快照，自动测试用专用假环境凭据，真实 Keychain 交互保留在人工验收。

## 发布与验证

一个仓库、一个 workspace，两端独立发布：`windows-v*` / `macos-v*` 分别触发各自 release workflow。不长期维护平台分支，Git 操作遵循 [AGENTS.md](../AGENTS.md) 的只读约定。

共享层或 FFI 改动需要两端完整回归；前端改动跑对应完整验收入口。自动检查与人工矩阵分别记录，编译通过不能替代窗口视觉、IME、登录自启、干净机器、签名或公证验收。

现行契约：macOS [桌面陪伴计划](plans/macos-companion-evolution.md)、[原生入口迁移](plans/native-ui-rewrite.md)，Windows [原生入口迁移](plans/windows-native-rewrite.md)。阶段进展和未关闭项以各自执行记录为准。

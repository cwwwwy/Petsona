# Petsona 原生功能状态汇总

契约：[计划 v1.0](plans/native-ui-rewrite.md)；事实与证据：[执行记录](execution/native-ui-rewrite.md)。
此表仅汇总当前原生实现，不以旧 egui 测试或计划中的类名作为已实现证据。
当前原生功能仍未达到完整验收；Windows 原生线已完成 B0–B5 自动化门禁，人工矩阵仍待闭合。

| 功能 | REQ | macOS 当前实现 | 自动/人工验证 | 缺口 / 审查项 |
|---|---|---|---|---|
| V1/V2资源与动画帧 | 03/05/07 | Rust worker 负责加载/校验，原生按快照绘制 V2 atlas；macOS 已有 AppKit WebP 读取回归和缩略图缓存，Windows 有 WIC/WebP 回退 | Mac XCTest8、Windows E-W14；真实视觉待人工 | Swift 宠物主窗口仍按 atlas path 读图，字节/缓存服务迁移仍是 REV-02 后续项 |
| 宠物库与空库首启 | 05/06 | worker 支持本地导入/覆盖/切换/删除/导出；macOS/Windows 原生设置均支持 Codex 预览、目录/zip 拖放、重复 ID 覆盖确认、导出/删除；macOS 本地列表已补首帧缩略图与缓存 | Rust library tests、Mac XCTest8、Windows E-W14、两端 smoke；视觉待人工 | 多屏等延期项不在本批次；M-01/M-W 仍需人工 |
| 注视、caret、跨行过渡 | 07/10 | 共享 Rust 使用官方 16 向映射；macOS 新增 GazeStabilizer、持续目标重发和拖动暂停注视；Windows 有 C# GazeStabilizer | Rust 状态测试、Mac XCTest7、Windows dotnet36、双方 smoke；视觉待人工 | Mac/Windows 迟滞与拖动观感仍需双方 M-W/M-02/M-04 |
| 状态协议与TTL | 07/12 | 新原生 app 通过 worker 执行 TTL，并刷新 `/health` | E-17/E-18 waiting→idle | 协议全字段与人工协议兼容待补 |
| 透明窗口与焦点 | 08 | 原生非激活 NSPanel；Composer 独立可激活 | E-15 编译；焦点/穿透待 M-02/M-04 | 多屏/Spaces 和实际前台规则待验 |
| 像素级穿透 | 08 | `PetView.hitTest` 当前帧 alpha + idle 行并集；整窗不再忽略鼠标 | Rust旧规则 + 原生编译；人工待验 | Retina alpha 坐标和桌面实际穿透待 M-02 |
| 单双击与拖动 | 08/09 | 原生区分单击/双击跳跃/拖动运行姿势并保存位置；输入入口为编辑按钮或气泡 | E-15 编译；人工待 M-02/M-03 | 原生事件肉眼验收尚未完成 |
| 缩放、位置、多屏、重力 | 09 | 原生固定缩放档位 0.5–2.0，设置与状态栏菜单共用；物理像素位置命令已接；重力/多屏延期 | runtime scale normalization、E-18；档位与保存待人工 | 连续缩放已移除；多屏/重力属于延期范围 |
| 自动活动/立即活动 | 09 | 原生菜单仍为基础立即 running；配置命令已接 worker | 自动尚未覆盖；人工未做 | auto-walk scheduler 与原生菜单完整接入待补 |
| 气泡/影子/编辑按钮 | 10 | 原生 BubblePanel 点击打开 Composer；宠物下方编辑按钮打开 Composer；气泡不再内置回复按钮 | E-15 编译；视觉待 M-04 | 影子到编辑动画属于延期范围 |
| 对话输入/IME/草稿 | 10/11 | 原生 NSTextView、Enter/Shift+Enter、Esc、草稿保留、后台 DeepSeek/fallback | E-15 XCTest worker 命令；IME/网络人工待 M-04 | caret 方向已接，候选确认和撤销需人工 |
| 设置、人格、记忆、DeepSeek | 11 | 原生设置支持 DeepSeek 全配置/Keychain、人格 CRUD/模板/导入导出、记忆配置/事实/事件清理；对话显式偏好自动入记忆并参与提示 | runtime projection/command tests、原生 EngineClient 往返测试；M-01/M-04 待人工 | 网络真实回复、IME 和长时间资源验收仍待；自动偏好只接受明确第一人称表达 |
| 托盘与菜单 | 12 | 原生 NSStatusItem 菜单含设置、宠物选择、固定缩放档位、显示隐藏、立即活动和退出；当前宠物有状态标识 | E-18；菜单交互待人工 | 宠物图标托盘化属于延期范围 |
| Keychain/LaunchAgent | 04/12 | Rust worker 已有 Keychain API/保存命令；旧 LaunchAgent 模板保留 | E-18 模板检查；原生设置接入待人工 | 原生 LaunchAgent 开关和 Keychain UI 待补 |
| 单实例/日志/退出 | 12/13 | worker 持有 InstanceLock，日志在 worker，FFI destroy join | E-14/E-17/E-18 | fault 注入、资源增长、5分钟 CPU 待 M-05/M-06 |
| FFI/线程/故障 | 01/02 | ABI 3 薄转换 + worker、ready/faulted、panic terminal 状态 | E-13/E-14/E-15 layout/lifecycle | panic 注入与跨语言 fault UI 待补 |
| 调度与性能 | 13 | 空库 worker 秒级 deadline；动画按 deadline 调度；原生 timer 使用快照 deadline | E-14/E-18 | Activity Monitor / 5分钟 CPU 待 M-06 |
| 测试与打包 | 14/15 | 统一脚本真实构建/测试/启动/打包 native app；静态 `.a` 检查 | E-18 exit 0；xcresult/zip/otool 证据 | 签名、公证、干净机器和人工窗口仍待 |
| Windows原生/全仓清理 | 16 | Windows B0–B5 已落地，旧 egui/旧 shell 仍保留；Mac/Windows 人工验收后再清理 | Windows E-W14 自动门禁通过；人工待验 | REQ-W15/B6 依赖两端验收闭合，当前不能删除旧共享 UI |

旧测试迁移时，在执行记录逐项填写“原测试 → 新测试 → 新入口证据”，不能仅保留测试数量。
状态变化必须关联新证据；不得通过改此表替代修改计划或补验收。

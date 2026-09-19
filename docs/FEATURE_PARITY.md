# Petsona 原生功能状态汇总

契约：[计划 v1.0](plans/native-ui-rewrite.md)；事实与证据：[执行记录](execution/native-ui-rewrite.md)。
此表仅汇总当前原生实现，不以旧 egui 测试或计划中的类名作为已实现证据。
当前所有原生功能均未达到完整验收；Windows 为后续范围，未开始原生实现。

| 功能 | REQ | macOS 当前实现 | 自动/人工验证 | 缺口 / 审查项 |
|---|---|---|---|---|
| V1/V2资源与动画帧 | 03/05/07 | Rust worker 负责加载/校验，原生按快照绘制 V2 atlas | E-15/E-18 自动通过；真实视觉待人工 | Swift 仍按 atlas path 读图，需迁移为字节/缓存服务；REV-02 部分 |
| 宠物库与空库首启 | 05/06 | worker 支持本地导入/覆盖/切换/删除/导出命令；空库原生自动打开设置 | E-15/E-18；导出/删除 UI 与 M-01 待人工 | 原生设置尚未提供导出/删除确认完整界面；REV-07 部分 |
| 注视、caret、跨行过渡 | 07/10 | FFI/worker 支持 caret dx/dy 与清除注视；原生 NSTextView 已接 caret 回调 | Rust 状态测试、E-18；中文 IME/视觉待人工 | 全局鼠标注视与 native UI 视觉待 M-02/M-04 |
| 状态协议与TTL | 07/12 | 新原生 app 通过 worker 执行 TTL，并刷新 `/health` | E-17/E-18 waiting→idle | 协议全字段与人工协议兼容待补 |
| 透明窗口与焦点 | 08 | 原生非激活 NSPanel；Composer 独立可激活 | E-15 编译；焦点/穿透待 M-02/M-04 | 多屏/Spaces 和实际前台规则待验 |
| 像素级穿透 | 08 | `PetView.hitTest` 当前帧 alpha + idle 行并集；整窗不再忽略鼠标 | Rust旧规则 + 原生编译；人工待验 | Retina alpha 坐标和桌面实际穿透待 M-02 |
| 单双击与拖动 | 08/09 | 原生区分单击延迟、双击打开 Composer、拖动运行姿势并保存位置 | E-15 编译；人工待 M-02/M-03 | 原生事件肉眼验收尚未完成 |
| 缩放、位置、多屏、重力 | 09 | 原生按快照调整 sprite 窗口大小；物理像素位置命令已接；重力/多屏仍待 | E-18 构建/smoke；人工待 M-03 | MonitorService、工作区夹取、重力调度尚未迁移 |
| 自动活动/立即活动 | 09 | 原生菜单仍为基础立即 running；配置命令已接 worker | 自动尚未覆盖；人工未做 | auto-walk scheduler 与原生菜单完整接入待补 |
| 气泡/影子/编辑按钮 | 10 | 原生 BubblePanel + hover 回复按钮；Composer 从宠物下方出现 | E-15 编译；视觉待 M-04 | 影子/按钮形态动画尚未完全对齐旧入口 |
| 对话输入/IME/草稿 | 10/11 | 原生 NSTextView、Enter/Shift+Enter、Esc、草稿保留、后台 DeepSeek/fallback | E-15 XCTest worker 命令；IME/网络人工待 M-04 | caret 方向已接，候选确认和撤销需人工 |
| 设置、人格、记忆、DeepSeek | 11 | 原生设置支持行为、人格 patch/save；worker 读记忆并后台生成回复 | E-18 health/命令基础；完整设置人工待 M-01/M-04 | 记忆事实编辑、DeepSeek key UI 尚未完整 |
| 托盘与菜单 | 12 | 原生 NSStatusItem 基础菜单、设置/显示隐藏/立即活动/退出 | E-18 通过协议 app；菜单外观待人工 | 当前菜单未列全部宠物/当前勾选；待补 |
| Keychain/LaunchAgent | 04/12 | Rust worker 已有 Keychain API/保存命令；旧 LaunchAgent 模板保留 | E-18 模板检查；原生设置接入待人工 | 原生 LaunchAgent 开关和 Keychain UI 待补 |
| 单实例/日志/退出 | 12/13 | worker 持有 InstanceLock，日志在 worker，FFI destroy join | E-14/E-17/E-18 | fault 注入、资源增长、5分钟 CPU 待 M-05/M-06 |
| FFI/线程/故障 | 01/02 | ABI 3 薄转换 + worker、ready/faulted、panic terminal 状态 | E-13/E-14/E-15 layout/lifecycle | panic 注入与跨语言 fault UI 待补 |
| 调度与性能 | 13 | 空库 worker 秒级 deadline；动画按 deadline 调度；原生 timer 使用快照 deadline | E-14/E-18 | Activity Monitor / 5分钟 CPU 待 M-06 |
| 测试与打包 | 14/15 | 统一脚本真实构建/测试/启动/打包 native app；静态 `.a` 检查 | E-18 exit 0；xcresult/zip/otool 证据 | 签名、公证、干净机器和人工窗口仍待 |
| Windows原生/全仓清理 | 16 | 后续范围 | 未做 | 当前不能删除仍被Windows依赖的旧共享UI |

旧测试迁移时，在执行记录逐项填写“原测试 → 新测试 → 新入口证据”，不能仅保留测试数量。
状态变化必须关联新证据；不得通过改此表替代修改计划或补验收。

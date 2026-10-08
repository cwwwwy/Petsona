# 原生功能与验收状态

当前macOS主线：[桌面陪伴计划](plans/macos-companion-evolution.md)与[执行记录](execution/macos-companion-evolution.md)。原生迁移历史：[计划](plans/native-ui-rewrite.md)与[执行记录](execution/native-ui-rewrite.md)。Windows：[计划](plans/windows-native-rewrite.md)与[执行记录](execution/windows-native-rewrite.md)。

本表只汇总当前实现与缺口，不替代计划契约或关闭验收项。macOS27.0.1 arm64最新自动证据为E-11（文档/脚本整理后的重跑）；最低支持26，macOS26实机仍待验。Windows早期W矩阵通过，W12暂缓，后续W29–W33及共享回归/发布剩余项继续按其清单验收。

| 功能 | macOS当前实现 | 自动证据 | 待验/限制 |
|---|---|---|---|
| V1/V2资源与动画 | 共享Rust加载/校验，AppKit atlas绘制与缓存；无内置宠物 | core状态测试、资源XCTest、native smoke | Codex锁定版本动作节奏逐项实测；主窗口仍按本地atlas路径读图 |
| 宠物库 | 本地导入/覆盖/导出/删除、拖放、Codex候选；单击选择、双击切换/导入 | 配置命令与空库测试、smoke | 两端双击、覆盖和切宠实际观感；Windows W33 |
| 注视/caret | 实际渲染矩形点坐标，1pt中心死区，16方向、跨行与持续目标更新 | Retina体内目标→FFI→Rust回归、core跨行测试 | Codex实际触发范围/过渡节奏、光标输入和缩放观感 |
| 点击/拖动/穿透 | 单双击分辨、拖动运行姿势、像素alpha与idle并集命中；非激活宠物窗 | core/几何/生命周期测试 | 真实鼠标/触控板、焦点、Retina命中与拖动即时性 |
| 缩放/屏幕/位置 | 固定缩放档位、物理坐标和显示器相对位置、可见工作区夹取 | DisplayGeometry XCTest | 多屏、混合Retina、热拔插和Spaces已纳入macOS当前范围，待实机；Windows W12仍按原计划暂缓 |
| 浮层/输入 | 原生玻璃、薄胶囊入口、气泡预览/点击历史、动态多行NSTextView、生成中保留草稿 | 浮层布局/焦点/多行/草稿XCTest | Codex材质/位置/动画对照、IME、选区、撤销、侧挂和悬停时序 |
| 聊天/历史 | SSE流式、等待/停止/失败重试、部分回复、按宠物保存/查看/清除 | 本地SSE stub、存储与请求校验测试 | 真实兼容端点、切宠/退出迟到结果、重启与IME的完整流程 |
| 记忆 | 多值偏好、否定/修正、手工冲突候选、习惯确认/拒绝、来源/时间、编辑/导入导出；与历史独立 | core提取/压缩/兼容、runtime隔离测试 | 中文表达质量、候选质量、开关/清除与跨宠物实机验收 |
| 人格 | 手动、粘贴/TXT/JSON聊天来源、人物参考；草稿编辑/试聊/应用，保留稳定ID与记忆 | parser、模型stub、旧JSON与稳定ID测试 | 超时/取消补测、试聊隔离、不同人格风格与重启人工对照；Windows新增UI另行规划 |
| 设置/菜单 | 系统原生六页、模型加载反馈；精简调参但保留旧值；菜单含设置、历史、宠物、缩放、显隐、退出 | 连续配置合并保留旧值测试；模型页界面观察 | Dock/快捷键/焦点、两端设置视觉；macOS“立即活动”和“测试问候”已移除 |
| 协议/单实例 | source优先级、TTL、同source clear；锁、日志、退出与端口释放 | core协议测试、FFI/runtime生命周期、smoke | GUI状态切换、故障界面、实际CPU和资源回收 |
| 凭据/自启 | 原生Keychain/LaunchAgent；自动测试用规范deepseek字段与假环境凭据、隔离登录项 | 凭据短路、运行时环境断言与服务XCTest | 真实Keychain和下一次登录自启；不会因为自动门禁通过而关闭 |
| FFI/线程/打包 | ABI3 worker、快照/JSON投影、静态Rust链接；常规包及保留本机数据的隔离验收包 | ABI/生命周期、33 XCTest、8 smoke、包结构与最低版本 | 共享Windows完整回归、签名/公证/干净机器与人工窗口 |
| 清理/维护 | 旧egui/shell、过期参考和缓存已清理；单一平台门禁、发布脚本、当前契约/执行证据保留 | 构建和打包不依赖删除文件 | 历史失败/待验状态保留，不能因清理文件而关闭旧问题 |

重力、自动活动提醒、语音、云同步和人物联网检索不属于当前macOS陪伴范围。

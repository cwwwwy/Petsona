# Petsona 原生重构文档入口

完整任务计划已迁至 [docs/plans/native-ui-rewrite.md](plans/native-ui-rewrite.md)。
本文件保留为兼容入口，不再维护另一份目标、范围或验收标准。

- 长期规则：[AGENTS.md](../AGENTS.md)
- 执行契约：[native-ui-rewrite，v1.0](plans/native-ui-rewrite.md)
- 实现、命令证据与审查问题：[执行记录](execution/native-ui-rewrite.md)
- 功能状态汇总：[FEATURE_PARITY.md](FEATURE_PARITY.md)
- 新任务模板：[计划模板](plans/TEMPLATE.md)、[执行模板](execution/TEMPLATE.md)

当前结论：未完成。已存在 FFI/macOS 前端骨架；原生静态链接、线程边界、TTL、
穿透、调度、故障状态与测试隔离等审查问题尚未修复。
此前构建/基础单测结果不能作为完整 macOS 验收，更不能作为整个跨平台重构完成的证据。

## 如何交给另一个对话

执行请求示例：

> 请读取 AGENTS.md、docs/plans/native-ui-rewrite.md 和对应执行记录。
> 按计划 v1.0 实施当前 macOS 与必要共享层范围，先复述关键目标和验收标准。
> 保留现有未提交改动，更新 docs/execution/native-ui-rewrite.md。
> 发现计划冲突时按暂停规则处理，不自行缩减目标或修改验收标准。

审查请求示例：

> 请对照 AGENTS.md、docs/plans/native-ui-rewrite.md 和对应执行记录审查实际 diff，
> 包括未跟踪文件；只检查计划完成度、架构回归和必要测试，不修改文件。
> 按 REQ/REV 编号报告位置、影响与证据。

本轮整理文档没有授权自动恢复代码实施；以上是后续对话可使用的请求示例。

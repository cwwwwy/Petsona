# Petsona 开发执行记录

本文档长期记录各开发阶段的实现、自动验证、人工验收和遗留事项。按阶段追加或更新对应条目；不要用后续阶段结果覆盖历史证据。

## 项目收束与 Git 历史重建（2026-10-13，已完成）

### 完成范围

- scripts 保留两端构建/回归四个入口；Windows 协议回归改为验证状态离开 waiting，并记录实际回退状态，避免把有效的 gaze 状态误判为 TTL/CLEAR 失败。
- docs 只保留当前计划与执行记录；架构、构建和用户说明归并在 AGENTS.md 与中英文 README。
- 删除过期开发文档、报告、归档、一次性脚本、pet_inspect 示例、空的旧 apps/macos Xcode 残留、旧安装包输出及重建前的 Rust/前端缓存。保留产品源码、测试夹具、锁文件、品牌源图和 CI。
- Mac 回归证据保留在 .scratch/macos/accept/50f144e2117d48cea7352a39c0734bc5/report.json；该目录是本次有效结果，不是旧归档。仓库内 target、node_modules、dist 和 pnpm 本地缓存已清理。

### 回归结果

- macOS 27.0.1/arm64：验收 debug 构建和原生回归通过。报告 binary SHA256 为 c6a861b80e6afa4de6bce2fed32b110e69f2f42b026afc5decb189d99603ef7，测试源提交为 a0a07d4c16e17cfa46d0fb460ae491ef44b5252f。覆盖宠物浮层、生命周期、协议 TTL/粘滞/非法请求/CLEAR、位置重启、空库、Composer 焦点和端口释放；没有注入键鼠。报告生成时的 pending 列表还包含 Windows shared regression；该项之后由用户确认通过。真实登录自启及 macOS 26/Intel/混合显示器专项仍待验。
- Windows：用户确认修正后的 release 回归通过。成功报告路径、二进制 SHA 和退出码未提供，不补造。
- Bash/Python 语法、PowerShell 文件 BOM 和 git diff --check 通过。Windows PowerShell 5.1 实际运行了修正后的回归脚本。

### GitHub 与本地历史收束

- 最终初始提交：efe8836e441bf72e75595f80c0537a662f9c5389，提交说明 Initial Petsona 0.1.0 baseline。
- 本地 Git 核对：main 与 origin/main 对齐；可达提交数为 1；本地无 tag 或临时测试分支；reflog 已清理；git fsck --no-reflogs --unreachable 未报告不可达对象。
- GitHub 只读核对：仅有未保护的 main 分支，SHA 为 efe8836e441bf72e75595f80c0537a662f9c5389；没有 tag 或 Release。仓库简介已更新，topics 为 codex、desktop-app、desktop-pet、llm、macos、rust、tauri、tray-icon、windows。
- 原始 refs：旧 main 87571e2e4166da230740cc15dd5ca10e0ddd3f02，旧 v0.1.0 tag 对象 d5dcb9945980be648aa88c881096ff95881e5119，旧 Release ID 385204836。
- GitHub 缓存、外部克隆及已下载副本不受本地 reflog/对象清理控制。

### 收尾核对

- scripts 当前 4 个入口；docs 以长期开发计划和开发执行记录为主；英文说明在 README.en.md。
- 本次最终检查发现 macOS Finder 又生成了 .DS_Store；已删除当前生成文件，.gitignore 会忽略后续重建的副本。
- 当前阶段项目目录与 Git 历史收束完成。M6-B 安装包、签名/公证、干净机器和安装升级验收尚未开始。

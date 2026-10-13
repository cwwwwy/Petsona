# 项目收束执行记录

## 本轮结果

- 用户已批准清理项目文件并进一步压缩脚本。保留两端构建/回归四个入口：desktop-build-macos.sh、desktop-build-windows.ps1、desktop-verify-macos.py、desktop-verify-windows.ps1。
- Windows 回归现在直接运行已构建的 debug 或 release 验收程序；隔离会话、窗口生命周期与无输入检查合入单一脚本。删除独立控制夹具、手动验收准备/启动器和项目结构清理脚本。
- macOS 回归移除手动登录/资料准备入口，只保留自动隔离回归。
- 删除 pet_inspect 开发 CLI 与其唯一源码引用。品牌源图、测试夹具、Cargo/pnpm 锁文件和产品构建配置保留。
- README 已合并现行架构、数据、构建和回归说明；AGENTS.md 维护协作与技术约束。docs 只保留当前计划/执行记录。
- 清理旧阶段计划、执行报告、TEMPLATE、归档、JSON证据和 .scratch 产物；本轮删除仓库内两个 target、node_modules、前端 dist、38 MiB 旧安装包输出、5 个 Finder 元数据文件及空的旧 macOS Xcode 项目残留。Rust/前端缓存可重建，下一次构建会重新编译并安装依赖。

## 验收状态

- Mac27.0.1/arm64（2026-10-13）：pnpm install --frozen-lockfile、bash scripts/desktop-build-macos.sh debug 均 exit 0；python3 scripts/desktop-verify-macos.py 在允许本机回环访问的环境 exit 0，报告 PASS。默认沙箱第一次因禁止 bind 回环端口返回 PermissionError，未改变数据后在放行环境重跑通过。报告：.scratch/macos/accept/50f144e2117d48cea7352a39c0734bc5/report.json；二进制 SHA256=c6a861b80e6afa4de6bce2fed32b110e69f2f42b026afac5decb189d99603ef7；sourceCommit=a0a07d4c16e17cfa46d0fb460ae491ef44b5252f。覆盖宠物浮层、生命周期、协议 TTL/粘滞/非法请求/CLEAR、位置重启、空库、Composer 焦点、端口释放；无输入注入。报告保留的待专项项：真实登录自启、macOS26/Intel/混合显示器。Windows共享回归后来由用户在临时测试分支上确认通过。回归结束后移除本轮重建的 node_modules、dist、target、Mac.app 和 pnpm 本地store，仅保留本次自动回归证据目录。
- Windows/WSL：用户确认修正后的 release 回归正常通过。成功报告路径、二进制 SHA 与命令退出码未随确认提供，因此不补录这些字段。
- 本轮未运行Rust测试套件。Mac构建与原生回归见上；Windows回归由用户在Windows宿主运行。
- 当前本地main为bcb88d5，工作区在记录本次结果前干净；本地origin/main仍指向87571e2，本地origin/codex/project-consolidation-test跟踪引用指向bcb88d5。GitHub实时refs需在执行前用git ls-remote再次确认；Mac此前DNS解析失败。AI没有执行提交、推送、分支/标签/Release删除或旧对象回收。
- 之前只读核对的 GitHub 状态：main=87571e2e4166da230740cc15dd5ca10e0ddd3f02；v0.1.0 tag 对象=d5dcb9945980be648aa88c881096ff95881e5119；旧 Release ID=385204836。用户执行前必须再次确认引用未变化。

## 用户执行的 Git 历史重建

Windows 回归已由用户确认通过。只在同步好的项目目录继续；先提交本执行记录，再移除旧 GitHub Release 和临时测试分支（不删除仓库）：

~~~sh
git add -A
if ! git diff --cached --quiet; then
  git commit -m "chore: consolidate project files"
fi
~~~

~~~sh
gh api --method DELETE repos/cwwwwy/Petsona/releases/385204836
git push origin --delete codex/project-consolidation-test
git fetch --prune origin
~~~

建立单提交初始历史并用固定 lease 原子替换 main、删除旧 tag；任一步失败都停止：

~~~sh
(
  set -e
  test "$(git branch --show-current)" = main
  git diff --check
  git add -A
  petsona_tree_before="$(git write-tree)"
  git checkout --orphan codex/fresh-start
  git add -A
  git commit -m "Initial Petsona 0.1.0 baseline"
  test "$(git rev-parse HEAD^{tree})" = "$petsona_tree_before"
  test "$(git rev-list --count HEAD)" = 1
  git branch -M main
  git push --atomic \
    --force-with-lease=refs/heads/main:87571e2e4166da230740cc15dd5ca10e0ddd3f02 \
    --force-with-lease=refs/tags/v0.1.0:d5dcb9945980be648aa88c881096ff95881e5119 \
    origin main:refs/heads/main :refs/tags/v0.1.0
  git tag -d v0.1.0
)
~~~

推送成功后，关闭 Codex 与其他 Git 写入工具，再清除本机旧对象：

~~~sh
git for-each-ref --format='delete %(refname)' refs/codex/turn-diffs | git update-ref --stdin
git reflog expire --expire=now --all
git gc --prune=now
git rev-list --all --count
git ls-remote --heads --tags origin
~~~

本机计数应为 1，远端应仅有 main 且没有旧 tag。其他工作副本重新克隆，不能把旧分支合并回新历史。历史重写不保证删除 GitHub 缓存、外部克隆或已下载副本。

同步 GitHub 仓库简介和主题（用户可在仓库 About 面板手动做同样修改）：

~~~sh
gh repo edit cwwwwy/Petsona \
  --description "Desktop pet for Windows & macOS — Rust/Tauri + TypeScript, Codex pet packs, chat, character and memory." \
  --remove-topic egui --remove-topic eframe --add-topic tauri
~~~

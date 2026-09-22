# windows-release-0.1.0-rc.1：执行与审查记录

## 接手

- 依据：2026-09-22 用户确认「全部按计划执行 + self-contained + 先发 `windows-v0.1.0-rc.1` pre-release」。
- 发布清单：`docs/WINDOWS_VERIFICATION.md` 的 **§R 发布前清单**（本记录为其执行证据）。
- 基线：`settings-consolidation` 与 `persona-memory-reshape` 的功能项均已实施并通过 Windows 人工复测（W15–W28）。

## 本轮改动

| 文件 | 改动 | 为什么 |
|---|---|---|
| `apps/windows/Petsona/Petsona.csproj` | `<WindowsAppSDKSelfContained>true` + `<SelfContained>true` | 让便携 zip 在干净机器上解压即用（用户拍板） |
| `Cargo.toml` / `Cargo.lock` | workspace version → `0.1.0-rc.1` | 首个 pre-release 的版本来源（tag 与 zip 名一致） |
| `scripts/verify-windows.ps1` / `scripts/package-windows.ps1` | UNC 根检测 → 传 `-p:WindowsAppSDKSelfContained=false -p:SelfContained=false` 并打印警告 | `mt.exe` 无法读取 `\\wsl.localhost\...`（实测 fail），开发机因此仍能跑门禁；自包含产物在本地副本 / CI 生成 |
| `.github/workflows/release-windows.yml` | 新增 `Create GitHub release`（`gh release create`，`-rc.` → pre-release） | 修掉 W-28（原来只 upload-artifact，没有下载页） |
| `CHANGELOG.md`（新） | 0.1.0-rc.1 条目：功能、已知限制、暂缓项 | 发布说明基础 |
| `README.md` | 新增「Install / upgrade / uninstall (Windows)」+ 隐私说明 | 便携包的自包含、升级保留数据、卸载三件事写清楚 |
| `crates/petsona-ffi/src/lib.rs` / `error.rs` | panic 分支先标 fault 再写 panic 文案（原来被 `engine handle is null` 覆盖）；`error::get` 改为仅测试使用；新增回归测试 | REV-05 的可见性与可测性 |
| `docs/WINDOWS_VERIFICATION.md` | 新增 §R 发布清单 | 把 12 项发布前事项变成可勾选清单 |

## 命令证据

| 证据ID | 内容 | 结果 |
|---|---|---|
| E-R1 | 本地副本自包含构建（`C:\Users\happyddz\AppData\Local\Temp\petsona-sc-rc1`） | `dotnet build -c Release -p:Platform=x64` **0 警告 0 错误**；输出 238 MB，含 `coreclr/hostfxr/hostpolicy/clrjit` 与 `Microsoft.WindowsAppRuntime.dll` 等 |
| E-R2 | 自包含产物运行验证 | 同一 smoke 套件指定 `-AppPath <自包含 exe>` → **25/25 PASS**（宠物窗 / 设置窗 / 协议 / 托盘 / 打包内容全通过） |
| E-R3 | `cargo test -p petsona-ffi` | **4/4**（新增 `panic_becomes_a_panic_status_instead_of_unwinding`） |
| E-R4 | `verify-windows.ps1 -Full` | cargo 全量 + dotnet **37/37** + FFI 守卫 + 打包结构通过；native smoke 首轮 **N18 FAIL**（桌面鼠标干扰，与代码无关）→ 单独重跑 **25/25 PASS** |
| E-R5 | 版本与锁文件 | `Cargo.toml` = `0.1.0-rc.1`，`Cargo.lock` 同步（`--locked` 通过） |

## 失败记录（保留）

- `mt.exe` 无法读取 UNC 路径：`WindowsAppSDK.SelfContained.targets(273,9) error MSB3073 ... c1010070: Failed to load and parse the manifest`，路径为 `\\?\wsl.localhost\...`。**结论**：自包含构建必须在本地盘或 CI 执行；脚本已按此回退。
- 升版本后首次 `cargo clippy --locked` 失败（lock 未更新）→ 先跑一次不带 `--locked` 的 `cargo check` 更新 `Cargo.lock`。
- `error::get` 一度成为死代码（panic 分支改用常量）→ 标记为 `#[cfg(test)]`。

## 待人工（发布前必须闭合）

1. **D1/D2**：用自包含产物确认无控制台窗口、图标正确（任务栏 / 资源管理器 / 快捷方式）。
2. **D5**：开启自启 → 注销 → 登录，确认自动启动。
3. **D6**：在一台干净 Windows（或新建本地用户）上解压运行 → 首启打开设置、能导入宠物、协议可用。
4. **首次 `workflow_dispatch`**：确认 release workflow 在 runner 上能自包含打包并创建 Release。
5. **打 tag 并发布 pre-release**：
   ```bash
   git tag windows-v0.1.0-rc.1
   git push origin windows-v0.1.0-rc.1
   ```
   然后到 GitHub 检查 Release（自动标记 pre-release）与 zip asset；下载回来再验一次。
6. **正式版**：rc 观察无阻塞问题后，把 `Cargo.toml` 升到 `0.1.0`，重复上述流程并打 `windows-v0.1.0`。

## 交付与接续

- 实际完成范围：R1–R7（清单见 §R）；R8–R12 待人工。
- 对用户数据的影响：无（仅版本号与打包方式变化）。
- Git 操作是否发生（默认无）：无。
- 完成判定：**未完成**（R8–R12 待人工，tag 与 GitHub Release 由用户执行）。

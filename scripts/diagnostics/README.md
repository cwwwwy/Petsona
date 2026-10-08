# Windows 原生界面截图

这两个脚本用于待验收的 Windows 设置页和浮层视觉检查，不加入门禁或 CI。
需要先运行 `scripts/verify-windows.ps1` 或 `scripts/package-windows.ps1` 构建原生程序。
截图使用临时 `PETSONA_HOME`，输出默认放在 `%TEMP%`；截图结果不能替代实机交互验收。

| 脚本 | 用途 | 关联验收 |
|---|---|---|
| `settings-shots.ps1` | 截取六个设置页面，可指定浅色/深色和输出目录 | W21/W22/W33；`SettingsWindow.SelectPage` 的现行诊断入口 |
| `overlay-shots.ps1` | 截取气泡和 Composer，可指定浅色/深色和输出目录 | W29–W32；`docs/execution/windows-overlay-interaction.md` E-OV05 |

```powershell
powershell -ExecutionPolicy Bypass -File scripts\diagnostics\settings-shots.ps1 -Theme dark
powershell -ExecutionPolicy Bypass -File scripts\diagnostics\overlay-shots.ps1 -Theme light
```

旧光标探针已被 `windows-smoke.ps1` N23/N24 完整覆盖，旧启动计时脚本是已完成基线测量的一次性工具，均于2026-10-04清理。历史结果保留在 `docs/execution/windows-native-rewrite.md` E-W15b/E-W15d；需要追溯源码时使用只读 `git show`。

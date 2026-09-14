> 当前决策（2026-09-14）：本项目暂不采用 WinUI3。为了实现“性能 + 系统原生美观”，
> 主方案改为同进程 Win32 原生菜单线程。本文件保留为未来 WinUI3 spike 的技术参考。
# Petsona WinUI3 菜单准备与方案

本文只讨论**同进程**方案，不采用独立 helper 进程。

## 0. 官方资料结论（2026-09-14）

- WinUI 3 官方文档明确说明其面向 C# 和 C++ 开发者，没有官方 Rust 应用开发支持：
  <https://learn.microsoft.com/en-us/windows/apps/winui/winui3/>
- WinUI3/Windows App SDK 不是 Windows 系统自带的普通 Win32 API。`windows`/`windows-rs`
  文档覆盖的是操作系统 Win32/WinRT API：
  <https://microsoft.github.io/windows-docs-rs/doc/windows/>
- 官方 XAML Island 宿主 API 是 `Microsoft.UI.Xaml.Hosting.DesktopWindowXamlSource`：
  <https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.hosting.desktopwindowxamlsource>
- 官方 Islands 示例使用 C++/WinRT 或 C#：
  <https://github.com/microsoft/WindowsAppSDK-Samples/tree/main/Samples/Islands>
- 非 MSIX/未打包应用必须在启动时初始化 Windows App SDK Runtime：
  <https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/use-windows-app-sdk-in-existing-project>
- 显式初始化可调用 `MddBootstrapInitialize`，官方教程：
  <https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/tutorial-unpackaged-deployment>
- crates.io 上没有成熟、官方、生产级的 Rust WinUI3 绑定；社区 crate 存在，但采用率、维护度
  和完整的 XAML Island 宿主能力都不足以替代官方 C++/WinRT 路径。

因此，“同进程 + WinUI3 + 性能最大化”的现实方案是：

```text
Rust/eframe 主线程
    -> C ABI
    -> 同进程 C++/WinRT shim DLL
    -> Windows App SDK bootstrap
    -> 常驻 STA/XAML dispatcher + DesktopWindowXamlSource
    -> 可复用的 WinUI3 MenuFlyout
```

## 1. 必须准备的内容

### 运行时与 SDK

- Windows App SDK Runtime，必须与目标架构一致（x64 至少；将来若有 ARM64 则对应 ARM64）。
- `Microsoft.WindowsAppSDK` 包。WinUI3 的 `Microsoft.UI.Xaml`、`Microsoft.UI.Content`、
  `Microsoft.UI.Windowing` 不属于 Windows 系统自带 API。
- 如果采用非 MSIX 分发，需要初始化 Windows App SDK bootstrap（`MddBootstrapInitialize` /
  对应自动初始化机制），并确保运行时已安装或使用 self-contained 部署。
- 如果采用 MSIX 分发，需要打包工程和对应身份/签名流程。

### 编译与宿主

- MSVC Build Tools + Windows SDK；若使用 C++/WinRT shim，还需要 C++/WinRT 支持。
- 或者 .NET SDK 8+ 与 C#/WinRT；但为了留在同一进程，最终仍需一个可被 Rust 调用的 shim DLL，
  或把 Rust 主程序改为承载 CLR。
- 推荐 C ABI shim DLL：
  - `menu_host_start()`
  - `menu_host_show(owner_hwnd, x, y, items)`
  - `menu_host_shutdown()`
  - 菜单命令通过 callback 或 channel 返回 Rust。

### XAML 宿主

- 独立 STA 线程。
- `DispatcherQueueController` 和 `DispatcherQueue`。
- `DesktopWindowXamlSource` / XAML Island。
- 一个轻量 HWND bridge 和 XAML root；`MenuFlyout` 必须附着在 XAML 元素上，不是任意 Win32
  窗口可直接调用的一次性 API。
- 菜单显示、关闭和命令回调都要在 XAML dispatcher 线程上执行。

## 2. 推荐架构

```text
Rust/eframe main thread
        |
        | C ABI / callback
        v
WinUI3 shim DLL (same process)
        |
        +-- persistent STA thread
        +-- DispatcherQueueController
        +-- DesktopWindowXamlSource / XAML Island
        +-- reusable MenuFlyout + MenuFlyoutItems
```

关键点：

- shim 和 XAML Island 只初始化一次，不要每次弹菜单都创建线程或 XAML 对象。
- `MenuFlyout` 和菜单项预构建并复用；只更新文字、启用状态和锚点。
- 主线程不等待 UI 线程；命令异步回到 Rust/eframe。
- WinUI3 shim 不参与 eframe 的 egui 绘制，只负责系统菜单窗口。

## 3. 性能最大化策略

- 常驻一个 Windows App SDK/XAML dispatcher 线程，避免每次菜单启动几百毫秒。
- 复用 `MenuFlyout`、`MenuFlyoutItem`、图标和图标转换结果。
- 使用简单 `MenuFlyoutItem`，不要在菜单里放复杂 XAML 控件。
- 让 XAML 自己负责高 DPI、主题和动画；不要混用 Win32 owner 的缩放计算。
- 确保全机只有一个 Windows App Runtime 版本，避免 bootstrap 时动态探测多个版本。
- 菜单出现前预warm XAML Island；菜单关闭后隐藏而不是销毁。
- 对菜单命令只发送轻量整数 id/枚举，不跨线程传复杂对象。

## 4. 风险和取舍

- WinUI3 XAML 是独立 dispatcher，和 eframe/winit 共存需要严格线程边界。
- XAML Island 需要 HWND bridge；多屏、DPI 和焦点行为必须单独验证。
- WinUI3 菜单可能比 Win32 `TrackPopupMenu` 更重，启动内存和首次弹出延迟更高。
- 会增加 Windows App Runtime 依赖、打包、签名和升级兼容性工作。
- 当前项目是 Rust-only、单 exe 形态；真实 WinUI3 同进程方案会打破这个形态。

## 5. 更轻的领先替代方案

如果目标只是“Windows 系统原生菜单”，而不是严格 WinUI3 视觉：

- 在同一进程创建专用 Win32 菜单线程。
- 使用 `CreatePopupMenu` / `AppendMenuW` / `TrackPopupMenuEx`。
- 使用 `TPM_RETURNCMD | TPM_WORKAREA`，命令 id 回传 eframe。
- 不增加运行时、不另起进程，系统自动处理主题、暗色模式、键盘和高 DPI。

该方案是当前架构下更低风险、更容易达到性能目标的路线；WinUI3 方案应作为独立 spike，
在 B5 和 Phase 2 多屏/位置记忆完成后再决定是否进入主构建。

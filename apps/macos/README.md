# Petsona macOS 原生前端

这是当前 macOS 验收入口，使用 SwiftUI/AppKit 调用共享 Rust runtime 的 C ABI。
应用包不内置宠物，也不会自动导入 `~/.codex/pets`；首次启动或本地库为空时会打开设置，
由用户点击“从 Codex 导入…”并选择具体宠物目录。

## 构建

```bash
xcodegen generate --spec apps/macos/project.yml --project apps/macos
cargo build -p petsona-ffi --release --locked
xcodebuild \
  -project apps/macos/Petsona.xcodeproj \
  -scheme Petsona \
  -configuration Release \
  -arch "$(uname -m)" \
  -derivedDataPath .scratch/macos-native-build \
  CODE_SIGNING_ALLOWED=NO build
```

## 直接打包

```bash
bash scripts/package-macos.sh dist
open dist/Petsona.app
```

也可以把 `dist/Petsona-macos-$(uname -m).zip` 复制到其他 Mac 解压后双击运行。
首次运行不需要开发工具；需要手动从设置导入宠物。

## 隔离验收

自动门禁和 native smoke：

```bash
bash scripts/verify-macos-all.sh
```

手动验收建议使用独立 `PETSONA_HOME`，避免修改真实配置、宠物库和端口。完整验收清单见
[`docs/MACOS_VERIFICATION.md`](../../docs/MACOS_VERIFICATION.md)，计划和逐项证据见
[`docs/plans/native-ui-rewrite.md`](../../docs/plans/native-ui-rewrite.md) 与
[`docs/execution/native-ui-rewrite.md`](../../docs/execution/native-ui-rewrite.md)。

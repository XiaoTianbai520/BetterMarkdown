# BetterMarkdown (Android Edition)

> Lightweight, privacy-first Markdown editor optimized exclusively for Android.

> [!NOTE]
> **BetterMarkdown (Android 专版)**
> 
> 本项目为仅面向 **Android 平台**的轻量化 Markdown 编辑器。已彻底清理移除所有与安卓无关的桌面端（macOS / Windows / Linux）、iOS、Web 官网站点及外围跨平台资产，仓库核心聚焦于 Android 移动端的高性能、轻量化 Markdown 写作与笔记管理。

[![License: MIT](https://img.shields.io/github/license/XiaoTianbai520/BetterMarkdown?color=orange)](LICENSE)
[![Platform: Android](https://img.shields.io/badge/platform-Android%207.0%2B-green.svg)](fdroid/)

---

## 核心特性 (Features)

- **纯本地优先 (Local-First)**：所有笔记保存在 Android 设备的存储目录中（支持系统存储访问框架 SAF 与应用私有目录）。
- **轻量与高性能 (Lightweight & Fast)**：针对移动端界面与交互优化，快速启动，低内存占用。
- **现代化 Markdown 体验**：即时渲染、LaTeX 公式支持、代码高亮、表格与任务列表。
- **AutoGit 版本控制**：内置轻量 Git 历史快照，无需安装外部系统 Git 即可实现每篇笔记的历史追溯与版本恢复。
- **暗色与亮色模式**：遵循 Material Design 规范，自适应系统深浅色切换与不同屏幕旋转方向。

---

## 项目结构 (Repository Structure)

```
.
├── app/          # 核心应用源码（Tauri 2 + 前端 Vue 3 + CodeMirror 6）
├── brand/        # 品牌与应用图标设计资产
├── design-icons/ # 设计与界面图标资源
├── docs/         # 核心技术文档与架构规格说明
├── fdroid/       # Android F-Droid 发布与打包元数据
└── scripts/      # Android 构建与自动化测试脚本
```

---

## Android 构建指南 (Building for Android)

### 前置环境 (Prerequisites)

- **Rust** (stable) 与 Android Target：
  ```bash
  rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
  ```
- **Node.js** (>= 18) 与 **pnpm**
- **Android SDK** (API 24+) 与 **Android NDK** (NDK 26+)
- **Java JDK** (JDK 17+)

### 快速构建 (Build)

1. 进入前端目录并安装依赖：
   ```bash
   cd app
   pnpm install
   ```

2. 运行构建脚本打包 Android APK / AAB：
   ```bash
   # 调试版本（跳过签名与 ProGuard）
   ./scripts/build-android.sh --debug

   # 正式签名发布包（需要配置 .env.local 密钥环境变量）
   ./scripts/build-android.sh
   ```

构建输出路径位于 `app/src-tauri/gen/android/app/build/outputs/`：
- `apk/`：包含 `arm64-v8a`, `armeabi-v7a`, `x86_64`, `universal` 架构 APK。
- `bundle/release/app-release.aab`：适用于 Google Play 的 App Bundle。

---

## 自动化测试 (Testing)

运行基于 ADB 的端到端 Android 回归测试：

```bash
./scripts/regress/android/run.sh
```

详细说明请参考 [scripts/regress/android/README.md](scripts/regress/android/README.md)。

---

## 开源协议 (License)

本项目遵循 [MIT 许可证](LICENSE)。

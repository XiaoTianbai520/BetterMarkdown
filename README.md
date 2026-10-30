# SoloMD (Android Edition)

> 轻量、纯净、本地优先的专属 Android 轻量化 Markdown 编辑器。

> [!NOTE]
> **专属 Android 轻量化改造**
> 
> 本项目已完全剪枝优化为**专属 Android 轻量化 Markdown 编辑器**：
> - **彻底移除商业与冗余组件**：移除所有桌面端（macOS / Windows / Linux）、iOS 代码与商业依赖，彻底删除 Google Play Billing 计费组件与相关权限，无任何商业广告与第三方追踪分析。
> - **精简权限与系统安全**：保留基础网络权限（`android.permission.INTERNET`，仅用于渲染 Markdown 文档中的远程外部图片）；绝不申请敏感存储权限（无需 `READ_EXTERNAL_STORAGE` / `MANAGE_EXTERNAL_STORAGE`），原生通过 Android 系统存储访问框架（Storage Access Framework, SAF）安全、合规地读写用户授权目录与文件。
> - **极致编译体积优化**：Rust 底层开启最高规格 release 优化（`opt-level = "z"`, `lto = "fat"`, `strip = "symbols"`），并去除无用 JNI/静态库引用，生成极致精炼的 `libapp_lib.so` 原生库。

[![License: MIT](https://img.shields.io/github/license/XiaoTianbai520/BetterMarkdown?color=orange)](LICENSE)
[![Platform: Android](https://img.shields.io/badge/platform-Android%207.0%2B%20(API%2024%2B)-green.svg)](app/)

---

## 核心特性 (Features)

- **纯本地优先 (Local-First)**：所有笔记完整保存在 Android 设备的存储目录中，支持系统 SAF（`ACTION_OPEN_DOCUMENT_TREE`）与文件关联查看。
- **文件关联直开**：在 Android 系统文件管理器中支持“用 SoloMD 打开”查看与编辑 `.md`、`.markdown`、`.mdown`、`.mkd` 及 `.txt` 文档。
- **轻量与高性能 (Lightweight & Fast)**：针对移动端手势与屏幕自适应深度调优，瞬时启动，极低内存占用。
- **现代 Markdown 写作**：CodeMirror 6 驱动，支持实时渲染、LaTeX 数学公式、代码高亮、GFM 任务清单与表格。
- **原生深浅色支持**：遵循 Material Design 规范，自适应系统深浅色切换及屏幕旋转与边缘防遮挡。

---

## 本地构建与调试指南 (Build & Debug Guide)

### 1. 前置环境要求 (Prerequisites)

- **Node.js**: >= 18.0 (推荐 Node 20+ 或 22+)
- **pnpm**: >= 9.0
- **Rust**: 稳定版 (Rust 1.77+ / stable) 并安装 Android 编译目标架构：
  ```bash
  rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
  ```
- **Android SDK & NDK**:
  - Android SDK Platform 36 (minSdk 24, compileSdk 36)
  - Android NDK r26+ (如 `26.3.11579264`)
  - 配置环境变量（例如 Windows PowerShell）：
    ```powershell
    $env:ANDROID_HOME = "C:\Users\<user>\AppData\Local\Android\Sdk"
    $env:NDK_HOME = "$env:ANDROID_HOME\ndk\26.3.11579264"
    $env:ANDROID_NDK_HOME = "$env:ANDROID_HOME\ndk\26.3.11579264"
    ```
- **Java JDK**: JDK 17 或 JDK 21

---

### 2. 构建命令 (Compilation Commands)

#### (1) 安装依赖与前端构建
```bash
cd app
pnpm install
pnpm build
```

#### (2) 构建 Android 调试包 (Debug APK)
你可以使用 Tauri CLI 一键编译打包：
```bash
# 在 app 目录下执行
pnpm tauri android build --debug --apk
```
或者直接使用生成的 Gradle 脚本进行打包：
```bash
cd app/src-tauri/gen/android
# Linux / macOS:
./gradlew assembleDebug

# Windows:
.\gradlew.bat assembleDebug
```

构建生成的 Debug APK 位于：
`app/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-universal-debug.apk`
（同时会在对应架构子目录生成 `arm64-v8a`、`armeabi-v7a`、`x86_64` 的独立瘦身包）。

#### (3) 构建正式发布包 (Release Build)
```bash
cd app
# 构建签名 Release APK 与 AAB（需要配置签名环境变量）
pnpm tauri android build --apk
pnpm tauri android build --aab
```

---

## 项目结构 (Repository Structure)

```
.
├── app/
│   ├── src/              # 前端界面源码（Vue 3 + TypeScript + CodeMirror 6）
│   └── src-tauri/        # Tauri 2 原生核心（Rust 后端与 Android JNI 桥接）
│       └── gen/android/  # Android 工程源码与 Gradle 配置
├── brand/                # 品牌与应用图标设计资产
├── design-icons/         # 设计图标资源
├── docs/                 # 架构与开发规范文档
├── fdroid/               # Android F-Droid 发布与打包元数据
└── scripts/              # Android 构建、回归测试与发布脚本
```

---

## 开源协议 (License)

本项目遵循 [MIT 许可证](LICENSE)。

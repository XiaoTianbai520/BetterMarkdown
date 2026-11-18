# ZeroMD (Android Edition)

> 轻量、纯净、本地优先的 Android 专属 Markdown 编辑器。
> Zero Bloatware, Zero Tracking, Zero Lock-in. Just Write.

[![License: MIT](https://img.shields.io/github/license/XiaoTianbai520/BetterMarkdown?color=orange)](LICENSE)
[![Platform: Android](https://img.shields.io/badge/platform-Android%207.0%2B%20(API%2024%2B)-green.svg)](app/)

---

## 项目简介

ZeroMD 是一个面向 Android 手机的单文档 Markdown 编辑器：启动即写，笔记以标准 `.md` / `.txt` 纯文本保存在你自己选择的本地文件夹里，不登录、不同步、不上传。

项目按 [docs/PRD-1.0.md](docs/PRD-1.0.md) 的 1.0 范围实现，**主动移除**了桌面端代码、AI/Agent、云同步、数据库式表格（Bases/属性）、导出与打印、主题商店、更新检查、遥测与内购等能力。PRD 第 3 节列出的"不做什么"是硬约束，后续迭代也以此为准。

技术栈：Tauri 2 (Mobile) + Rust、Vue 3 + TypeScript + Pinia、CodeMirror 6、markdown-it、KaTeX、highlight.js。

---

## 主要功能

| 模块 | 说明 |
| --- | --- |
| 文件库（Vault） | 通过 Android SAF（`ACTION_OPEN_DOCUMENT_TREE`）选择任意本地文件夹，持久授权，无需任何存储权限 |
| 文件树抽屉 | 左侧滑出，多级文件夹折叠/展开，点击载入文档；顶部可切换目录库 |
| 编辑 / 预览 | 顶栏一键切换源码编辑与渲染预览；CodeMirror 6 触控优化，默认软换行 |
| 键盘辅助条 | 软键盘上方常驻 粗体 / 斜体 / 标题 / 无序列表 / 任务 / 代码块 / 链接 / 撤销 / 重做 / 收起键盘，点击不丢焦点 |
| Markdown 渲染 | GFM（标题/强调/删除线/引用/列表/表格/任务列表）+ 行内与块级 KaTeX + 代码高亮；任务列表可在预览中直接勾选回写 |
| 保存 | 顶栏软盘按钮手动保存；更多菜单提供「保存 / 另存为…」；另有自动保存（默认 10 秒，可关闭）与切后台自动保存 |
| 全局搜索 | 顶栏放大镜，跨当前文件库检索并跳转 |
| 设置 | 字号 12–28px、行高 1.2–2.4、自动换行、自动保存间隔（关闭/5/10/30/60 秒）、主题（跟随系统/浅色/深色）、语言（跟随系统/简体中文/English）、关于 |

已知的降级行为：`mermaid`、`plantuml` 等图表代码块按普通代码块高亮展示，不做渲染（PRD 明确不引入重型图表引擎）。

---

## 环境要求

- **Node.js** ≥ 20（推荐 22 LTS）
- **pnpm** ≥ 9
- **Rust** stable（1.77+），并安装 Android 目标：
  ```bash
  rustup target add aarch64-linux-android
  ```
  当前构建只产出 `arm64-v8a`（`build.gradle.kts` 中 `abiFilters += "arm64-v8a"`），因此只需这一个 target；如需 32 位包再补 `armv7-linux-androideabi`。
- **Android SDK**：Platform 36、Build-Tools 36、Platform-Tools（`adb`）
- **Android NDK**：r26+（例如 `26.3.11579264`）
- **JDK**：17 或 21

环境变量（Windows PowerShell 示例）：

```powershell
$env:ANDROID_HOME     = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_NDK_HOME = "$env:ANDROID_HOME\ndk\26.3.11579264"
$env:JAVA_HOME        = "C:\Program Files\Java\jdk-21"
```

> Tauri CLI 在 `ANDROID_HOME` 未设置时会尝试自动定位已安装的 SDK；显式设置更稳妥。

---

## 安装与运行

```bash
cd app
pnpm install
```

### 开发调试

```bash
# 纯前端（浏览器，http://localhost:1420）
pnpm dev

# 真机 / 模拟器热调试（需要已连接设备，adb devices 可见）
pnpm tauri android dev
```

### 构建 APK

```bash
cd app

# 前端产物（先做类型检查，再打包到 app/dist）
pnpm build

# Release APK（当前只出 arm64-v8a）
pnpm tauri android build --apk --target aarch64
```

产物路径：

```
app/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk
```

构建 Debug 包：

```bash
pnpm tauri android build --apk --debug --target aarch64
```

也可以直接用 Gradle（在 `app/src-tauri/gen/android` 下）：

```powershell
.\gradlew.bat assembleRelease   # Windows
./gradlew assembleRelease       # macOS / Linux
```

> **每次构建前建议清一次 Gradle 产物目录**，否则前端 `app/dist` 或 Rust `.so` 可能不会重新嵌入：
>
> ```powershell
> Remove-Item -Recurse -Force app\src-tauri\gen\android\app\build
> ```
>
> ```bash
> rm -rf app/src-tauri/gen/android/app/build
> ```

---

## 配置说明

### 签名（Release）

在项目根目录或 `app/` 下放 `.env.local`，提供四个变量（`scripts/build-android.sh` 会自动 `source`；直接跑 `pnpm tauri` 时需自行导出为环境变量）：

```bash
ANDROID_KEYSTORE_PATH=/absolute/path/to/release.jks
ANDROID_KEYSTORE_PASS=******
ANDROID_KEY_ALIAS=zeromd
ANDROID_KEY_PASS=******
```

缺失时 `build.gradle.kts` 会回退到 **debug keystore** 签名：可以安装试用，但不能上架，且与已安装的正式签名包存在签名冲突（需先卸载）。可用 `apksigner verify --print-certs <apk>` 确认签名主体。

### 关键配置文件

| 文件 | 作用 |
| --- | --- |
| `app/src-tauri/tauri.conf.json` | 产品名、版本、`identifier`（`app.zeromd`）、前端产物目录、窗口与权限配置；**版本号以此文件为准** |
| `app/src-tauri/gen/android/app/build.gradle.kts` | `namespace` / `applicationId`、`abiFilters`、版本号、签名配置 |
| `app/src-tauri/gen/android/app/proguard-rules.pro` | 保留 `app.zeromd.MainActivity` 及其 `Companion` —— SAF 的 `@JvmStatic` 方法只被 Rust 通过 JNI 调用，R8 看不到 Java 引用，keep 规则写错包名会在 release 包里被整段裁掉 |
| `app/src-tauri/src/saf_android.rs` | JNI 桥，`CLS` 必须等于 `applicationId` 对应的 `MainActivity`（当前 `app/zeromd/MainActivity`） |
| `app/src-tauri/gen/android/app/src/main/AndroidManifest.xml` | 权限（仅 `INTERNET`）、Activity、deep-link intent-filter |

> **改包名时的三处同步**：`build.gradle.kts` 的 `applicationId`、`proguard-rules.pro` 的 keep 规则、`saf_android.rs` 的 `CLS`，以及 `MainActivity.kt` 的 `package` 声明。少一处就会出现「点打开文件夹没反应 / Java exception was raised」。

### 应用内设置

设置保存在 WebView 的 `localStorage`（键前缀 `zeromd.`），不入库、不上传。清除应用数据会重置为默认值。

---

## 使用示例

1. **首次打开文件库**：顶栏「⋯」→「打开目录库」→ 系统文件夹选择器 → 选一个存放 Markdown 的文件夹 →「允许访问」。
2. **新建并保存**：顶栏「+」新建 → 输入内容 → 点顶栏软盘（首次保存会弹系统保存对话框，选定文件名与位置）。标题旁的橙色圆点表示有未保存改动。
3. **预览与任务列表**：顶栏「眼睛」切到预览；GFM 表格、`$E=mc^2$` 公式、代码块高亮生效，点击 `- [ ]` 可直接勾选并回写源文件。
4. **搜索**：顶栏放大镜 → 输入关键词 → 点击结果跳转到对应文档。
5. **换库**：抽屉顶部「切换库」，或「⋯」→「打开目录库」，重新授权另一个文件夹。
6. **调整书写体验**：「⋯」→「设置」→ 编辑器偏好调字号/行高/自动保存间隔；外观里选跟随系统或固定深浅色。

---

## 项目结构

```
.
├── app/
│   ├── src/                    # 前端（Vue 3 + TS + CodeMirror 6）
│   │   ├── components/         # Editor / Preview / FileTree / GlobalSearch / SettingsPanel / KeyboardBar …
│   │   ├── composables/        # useFiles / useFileWatcher / useGlobalSearch / useViewport …
│   │   ├── lib/                # Markdown 渲染、KaTeX、高亮、SAF、CodeMirror 扩展
│   │   ├── stores/             # settings / workspace / tabs / toasts
│   │   ├── i18n/               # zh-CN / en-US
│   │   └── styles/
│   └── src-tauri/              # Tauri 2 Rust 核心
│       ├── src/                # lib.rs / commands.rs / saf_android.rs / search.rs / watcher.rs / workspace_index.rs
│       └── gen/android/        # Android 工程（Manifest、Gradle、MainActivity.kt）
├── docs/PRD-1.0.md             # 产品范围与硬约束（最高基线）
├── fdroid/metadata/app.zeromd.yml
├── scripts/                    # 构建与 Android 回归脚本
└── brand/                      # 图标与品牌资产
```

---

## 测试

```bash
cd app
pnpm test        # Node 内置 test runner，需 Node ≥ 22.6（--experimental-strip-types）
```

`scripts/regress/android/` 下有一套基于 `adb` 的真机回归脚本（包名 `app.zeromd`），默认从 `scripts/regress/android/run.sh` 进入，需要已连接设备或可用 AVD。

> `scripts/build-android.sh` 是按 macOS 环境写的（`HOST_TAG=darwin-x86_64`、`/opt/homebrew` 路径），**Windows 上不能直接用**，请按上面的 `pnpm tauri android build` 命令构建。

---

## 注意事项

- **只产出 arm64-v8a**：当前 Gradle 配置固定 `abiFilters = arm64-v8a`，32 位设备装不上；需要时再放开。
- **构建缓存**：Gradle 不会因为只改了前端或 Rust 就重新嵌入产物，务必先清 `gen/android/app/build` 再打包。
- **R8 与 JNI**：release 包会裁掉没有被 Java 引用的方法；`proguard-rules.pro` 的 keep 规则必须与实际包名一致，否则「打开文件夹」等 SAF 操作会在 release 里静默失效（debug 不受影响）。
- **签名**：没有 `.env.local` 时产物是 debug 签名，仅供试用；覆盖安装前注意签名一致性。
- **无存储权限**：应用不申请 `READ_EXTERNAL_STORAGE` / `MANAGE_EXTERNAL_STORAGE`，所有文件访问都走 SAF 授权；未授权目录无法读写。
- **功能边界**：不支持账号/云同步、AI、Bases/属性、导出（PDF/DOCX/EPUB）、打印、插件与主题商店 —— 这是 PRD 的刻意取舍，不是缺失。
- **图表降级**：Mermaid / PlantUML 代码块按源码高亮显示，不做渲染。
- **外部打开文件（待补齐）**：`AndroidManifest.xml` 里注册了 `content://` / `file://` 的 VIEW intent，但当前前端还没有监听 deep-link，从系统文件管理器选择 ZeroMD 只会把应用拉到前台，不会载入该文件（PRD 6.2 的外部单文件模式尚未接线）。
- **键盘条图片按钮**：精简时移除了 `editor.insertImage` 命令，键盘条上的图片按钮目前没有接收方；插入图片请用编辑器内的粘贴/文件处理路径，后续会一并清理或重新接线。

---

## 开源协议

本项目遵循 [MIT 许可证](LICENSE)。

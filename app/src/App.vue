<script setup lang="ts">
import {
  ref,
  computed,
  watch,
  watchEffect,
  onMounted,
  onBeforeUnmount,
  provide,
} from 'vue';
import { invoke } from '@tauri-apps/api/core';

// Phone status bar clock (Markor-style)
const phoneTime = ref('');
function updatePhoneTime() {
  const now = new Date();
  const h = String(now.getHours()).padStart(2, '0');
  const m = String(now.getMinutes()).padStart(2, '0');
  phoneTime.value = `${h}:${m}`;
}
let clockTimer: ReturnType<typeof setInterval> | null = null;

// Stores & Composables
import { useTabsStore } from './stores/tabs';
import { useSettingsStore } from './stores/settings';
import { useWorkspaceStore } from './stores/workspace';
import { useFiles } from './composables/useFiles';
import { useFileWatcher } from './composables/useFileWatcher';
import { useShortcuts } from './composables/useShortcuts';
import { useAutoCommit } from './composables/useAutoCommit';
import { useI18n } from './i18n';
import { baseNameOf, fileNameOf } from './lib/import-plan';
import { dataThemeFor } from './lib/themes';

// Components
import Editor from './components/Editor.vue';
import Preview from './components/Preview.vue';
import FileTree from './components/FileTree.vue';
import AndroidFolderPicker from './components/AndroidFolderPicker.vue';
import GlobalSearch from './components/GlobalSearch.vue';
import SettingsPanel from './components/SettingsPanel.vue';
import AboutDialog from './components/AboutDialog.vue';
import KeyboardBar from './components/KeyboardBar.vue';
import Toast from './components/Toast.vue';
import UnsavedDialog from './components/UnsavedDialog.vue';
import FileChangedDialog from './components/FileChangedDialog.vue';
import Icons from './components/Icons.vue';

const tabs = useTabsStore();
const settings = useSettingsStore();
const workspace = useWorkspaceStore();
const files = useFiles();
const autoCommit = useAutoCommit();
const { t } = useI18n();

autoCommit.start();

// Single document state
const currentFile = computed(() => tabs.activeTab);
const currentFileName = computed(() => {
  if (!currentFile.value) return t('common.untitled') || '未命名笔记';
  if (currentFile.value.fileName) return currentFile.value.fileName;
  if (currentFile.value.filePath) return fileNameOf(currentFile.value.filePath);
  return t('common.untitled') || '未命名笔记';
});
const isDirty = computed(() => (currentFile.value ? tabs.isDirty(currentFile.value.id) : false));

// Vault Display
const vaultDisplayName = computed(() => {
  const folder = workspace.currentFolder;
  if (!folder) return '选择目录库…';
  return baseNameOf(folder) || folder;
});

// Edit / Preview mode
const isPreviewMode = computed(() => settings.viewMode === 'preview');
function toggleEditPreview() {
  if (settings.viewMode === 'preview') {
    settings.setViewMode('edit');
  } else {
    settings.setViewMode('preview');
  }
}

// Drawer state
const drawerOpen = ref(false);

// Modals state
const searchOpen = ref(false);
const settingsOpen = ref(false);
const aboutOpen = ref(false);
const moreMenuOpen = ref(false);
const androidPickerOpen = ref(false);
const androidRestartNeeded = ref(false);

function openFolderPicker() {
  androidPickerOpen.value = true;
}

function onAndroidFolderPick(path: string) {
  androidPickerOpen.value = false;
  workspace.setFolder(path);
  drawerOpen.value = true;
}

async function requestAndroidStorage() {
  const toasts = (await import('./stores/toasts')).useToastsStore();
  try {
    localStorage.setItem('solomd:android-storage-pending', '1');
    await invoke('android_request_all_files_access');
    toasts.info(t('toast.androidAllFilesReturn') || '请在系统设置中允许所有文件访问权限');
  } catch (e) {
    toasts.error(String(e));
  }
}

async function doAndroidRestart() {
  androidRestartNeeded.value = false;
  try {
    await invoke('android_restart_app');
  } catch (e) {
    const toasts = (await import('./stores/toasts')).useToastsStore();
    toasts.error(String(e));
  }
}

// New note
function handleNewNote() {
  drawerOpen.value = false;
  files.newFile();
  if (settings.viewMode === 'preview') {
    settings.setViewMode('edit');
  }
}

// Unsaved changes dialog
const unsavedOpen = ref(false);
const unsavedMode = ref<'tab' | 'window'>('tab');
const unsavedFileName = ref('');
const unsavedCount = ref(0);
let unsavedResolve: ((action: 'save' | 'discard' | 'cancel') => void) | null = null;

function showUnsavedDialog(
  mode: 'tab' | 'window',
  fileName: string,
  count: number,
): Promise<'save' | 'discard' | 'cancel'> {
  unsavedMode.value = mode;
  unsavedFileName.value = fileName;
  unsavedCount.value = count;
  unsavedOpen.value = true;
  return new Promise((resolve) => {
    unsavedResolve = resolve;
  });
}

function onUnsavedAction(action: 'save' | 'discard' | 'cancel') {
  unsavedOpen.value = false;
  if (unsavedResolve) {
    unsavedResolve(action);
    unsavedResolve = null;
  }
}

provide('showUnsavedDialog', showUnsavedDialog);

// External file change dialog
const fileChangedOpen = ref(false);
const fileChangedName = ref('');
let fileChangedResolve: ((action: 'reload' | 'overwrite' | 'cancel') => void) | null = null;

function showFileChangedDialog(fileName: string): Promise<'reload' | 'overwrite' | 'cancel'> {
  fileChangedName.value = fileName;
  fileChangedOpen.value = true;
  return new Promise((resolve) => {
    fileChangedResolve = resolve;
  });
}

function onFileChangedAction(action: 'reload' | 'overwrite' | 'cancel') {
  fileChangedOpen.value = false;
  if (fileChangedResolve) {
    fileChangedResolve(action);
    fileChangedResolve = null;
  }
}

useFileWatcher(showFileChangedDialog);

// Hardware keyboard shortcuts (optional helper)
useShortcuts({
  openSettings: () => (settingsOpen.value = true),
  openGlobalSearch: () => (searchOpen.value = true),
});

// Editor events
const cursorLine = ref(1);
const cursorCol = ref(1);
const selectionText = ref('');
function onCursor(line: number, col: number) {
  cursorLine.value = line;
  cursorCol.value = col;
}
function onSelection(text: string) {
  selectionText.value = text;
}

// Auto-save interval runner
watchEffect((onCleanup) => {
  const intervalSec = settings.autoSaveInterval;
  if (intervalSec > 0) {
    const timer = setInterval(() => {
      void files.autoSaveDirtyTabs();
    }, intervalSec * 1000);
    onCleanup(() => clearInterval(timer));
  }
});

// Auto-persist active document
watch(
  () => [currentFile.value?.content, currentFile.value?.filePath, currentFile.value?.fileName],
  () => {
    tabs.persist?.();
  },
);

// Theme and font synchronisation
watchEffect(() => {
  document.documentElement.setAttribute(
    'data-theme',
    dataThemeFor(settings.theme, !!settings.customCssPath),
  );
});
watchEffect(() => {
  document.documentElement.style.setProperty('--content-font-size', `${settings.fontSize}px`);
});
watchEffect(() => {
  document.documentElement.style.setProperty(
    '--editor-line-height',
    String(settings.lineHeight || 1.6),
  );
});

function onFileOpened() {
  drawerOpen.value = false;
}

function onWindowBlur() {
  void files.autoSaveDirtyTabs();
}

onMounted(async () => {
  // Initialize phone status bar clock
  updatePhoneTime();
  clockTimer = setInterval(updatePhoneTime, 30000);

  // If no note is open, create a default Untitled note
  if (tabs.tabs.length === 0) {
    tabs.newTab({ fileName: 'Untitled.md', language: 'markdown' });
  }

  // Check Android storage access resumption
  if (localStorage.getItem('solomd:android-storage-pending')) {
    try {
      const hasPerm = await invoke<boolean>('android_check_all_files_access');
      if (hasPerm) {
        localStorage.removeItem('solomd:android-storage-pending');
        androidRestartNeeded.value = true;
      }
    } catch {}
  }

  window.addEventListener('solomd:file-opened', onFileOpened);
  window.addEventListener('blur', onWindowBlur);
});

onBeforeUnmount(() => {
  if (clockTimer) clearInterval(clockTimer);
  window.removeEventListener('solomd:file-opened', onFileOpened);
  window.removeEventListener('blur', onWindowBlur);
});
</script>

<template>
  <div class="mobile-app" :data-theme="dataThemeFor(settings.theme, !!settings.customCssPath)">
    <!-- 0. Phone Status Bar (Markor-style) -->
    <div class="phone-statusbar">
      <span class="phone-statusbar__time">{{ phoneTime }}</span>
      <div class="phone-statusbar__right">
        <svg class="phone-statusbar__icon" viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
          <path d="M12 3C7.95 3 4.21 4.34 1.2 6.6c3.41 4.56 6.07 7.46 7.63 9.23.78.88 1.28 1.45 1.7 1.66.42.21 1.05.21 1.47 0 .42-.21.92-.78 1.7-1.66 1.56-1.77 4.22-4.67 7.63-9.23C19.79 4.34 16.05 3 12 3z" opacity="0.6"/>
          <circle cx="12" cy="12" r="2"/>
        </svg>
        <span class="phone-statusbar__signal">5G</span>
        <svg class="phone-statusbar__wifi" viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
          <path d="M12 18l3.5-4.5c-1-0.8-2.2-1.3-3.5-1.3s-2.5 0.5-3.5 1.3L12 18z" opacity="0.8"/>
          <path d="M12 12c2.2 0 4.2 0.8 5.8 2.2l1.4-1.8C17.2 10.6 14.7 9.5 12 9.5s-5.2 1.1-7.2 2.9l1.4 1.8C7.8 12.8 9.8 12 12 12z" opacity="0.6"/>
          <path d="M12 7c3.3 0 6.4 1.2 8.8 3.2l1.2-1.6C19.3 6.2 15.8 4.8 12 4.8s-7.3 1.4-10 3.8l1.2 1.6C5.6 8.2 8.7 7 12 7z" opacity="0.4"/>
        </svg>
        <div class="phone-statusbar__battery">
          <div class="phone-statusbar__battery-body">
            <div class="phone-statusbar__battery-fill"></div>
          </div>
          <div class="phone-statusbar__battery-cap"></div>
          <span class="phone-statusbar__battery-text">79</span>
        </div>
      </div>
    </div>

    <!-- 1. Mobile Top Bar (Markor-style: minimal, filename + unsaved dot) -->
    <header class="mobile-topbar">
      <div class="mobile-topbar__left">
        <button
          class="topbar-btn"
          aria-label="打开目录树"
          title="目录"
          @click="drawerOpen = !drawerOpen"
        >
          <Icons name="sidebar" :size="20" />
        </button>
        <div class="topbar-title-wrap">
          <span class="topbar-title" :title="currentFileName">
            {{ currentFileName }}
          </span>
          <span v-if="isDirty" class="topbar-dirty-dot" title="未保存" />
        </div>
      </div>

      <div class="mobile-topbar__right">
        <!-- Edit / Preview Switch -->
        <button
          class="topbar-btn"
          :class="{ 'topbar-btn--active': isPreviewMode }"
          :title="isPreviewMode ? '切换到编辑模式' : '切换到预览模式'"
          @click="toggleEditPreview"
        >
          <Icons :name="isPreviewMode ? 'pencil' : 'eye'" :size="20" />
        </button>

        <!-- Global Search -->
        <button
          class="topbar-btn"
          title="全文搜索"
          @click="searchOpen = true"
        >
          <Icons name="search" :size="20" />
        </button>

        <!-- New Note -->
        <button
          class="topbar-btn"
          title="新建笔记"
          @click="handleNewNote"
        >
          <Icons name="plus" :size="20" />
        </button>

        <!-- More Actions Menu -->
        <div class="more-menu-wrapper">
          <button
            class="topbar-btn"
            title="更多操作"
            @click="moreMenuOpen = !moreMenuOpen"
          >
            <Icons name="more" :size="20" />
          </button>

          <!-- Dropdown Popover -->
          <div
            v-if="moreMenuOpen"
            class="more-menu-scrim"
            @click="moreMenuOpen = false"
          />
          <div v-if="moreMenuOpen" class="more-menu-dropdown">
            <button class="more-menu-item" @click="openFolderPicker(); moreMenuOpen = false">
              <Icons name="folder" :size="18" />
              <span>打开目录库</span>
            </button>
            <button class="more-menu-item" @click="settingsOpen = true; moreMenuOpen = false">
              <Icons name="settings" :size="18" />
              <span>设置</span>
            </button>
            <button class="more-menu-item" @click="aboutOpen = true; moreMenuOpen = false">
              <Icons name="info" :size="18" />
              <span>关于</span>
            </button>
          </div>
        </div>
      </div>
    </header>

    <!-- 2. Side Drawer (File Tree) -->
    <Transition name="drawer-fade">
      <div
        v-if="drawerOpen"
        class="drawer-scrim"
        @click="drawerOpen = false"
      />
    </Transition>
    <aside
      class="drawer-sidebar"
      :class="{ 'drawer-sidebar--open': drawerOpen }"
    >
      <div class="drawer-header">
        <div class="drawer-vault-meta" :title="workspace.currentFolder || '未选择库'">
          <Icons name="folder-sm" :size="18" class="drawer-vault-icon" />
          <span class="drawer-vault-name">{{ vaultDisplayName }}</span>
        </div>
        <button
          class="drawer-switch-btn"
          @click="openFolderPicker"
        >
          切换库
        </button>
      </div>

      <div class="drawer-tree-container">
        <FileTree :embedded="true" />
      </div>
    </aside>

    <!-- 3. Main Editor / Reader Viewport -->
    <main class="main-viewport">
      <template v-if="currentFile">
        <Preview
          v-if="isPreviewMode"
          :source="currentFile.content"
          :file-path="currentFile.filePath"
          :tab-id="currentFile.id"
        />
        <Editor
          v-else
          :key="currentFile.id"
          :tab="currentFile"
          :focus-mode="settings.focusMode"
          :typewriter-mode="settings.typewriterMode"
          :spell-check="settings.spellCheck"
          @cursor="onCursor"
          @selection="onSelection"
        />
      </template>
      <div v-else class="empty-viewport">
        <Icons name="file" :size="48" class="empty-icon" />
        <p class="empty-text">没有打开的文档</p>
        <button class="empty-action-btn" @click="handleNewNote">
          <Icons name="plus" :size="18" />
          <span>新建笔记</span>
        </button>
      </div>
    </main>

    <!-- 4. Soft Keyboard Auxiliary Formatting Bar -->
    <KeyboardBar />

    <!-- 5. Overlays and Dialogs -->
    <div v-if="searchOpen" class="search-modal-scrim" @click.self="searchOpen = false">
      <div class="search-modal-box">
        <GlobalSearch @close="searchOpen = false" />
      </div>
    </div>

    <AndroidFolderPicker
      :open="androidPickerOpen"
      :start="workspace.currentFolder ?? undefined"
      @pick="onAndroidFolderPick"
      @close="androidPickerOpen = false"
      @request-permission="androidPickerOpen = false; requestAndroidStorage()"
    />

    <SettingsPanel
      :open="settingsOpen"
      @close="settingsOpen = false"
    />

    <AboutDialog
      :open="aboutOpen"
      @close="aboutOpen = false"
    />

    <Toast />

    <UnsavedDialog
      :open="unsavedOpen"
      :file-name="unsavedFileName"
      :count="unsavedCount"
      :mode="unsavedMode"
      @save="onUnsavedAction('save')"
      @discard="onUnsavedAction('discard')"
      @cancel="onUnsavedAction('cancel')"
    />

    <FileChangedDialog
      :open="fileChangedOpen"
      :file-name="fileChangedName"
      @reload="onFileChangedAction('reload')"
      @overwrite="onFileChangedAction('overwrite')"
      @cancel="onFileChangedAction('cancel')"
    />

    <!-- Android Storage Permission Restart Notice -->
    <div v-if="androidRestartNeeded" class="arn-backdrop">
      <div class="arn">
        <div class="arn__title">重启一次即可读取文件夹</div>
        <p class="arn__body">
          文件访问权限已开启 ✅。安卓要求 App 重启后才能真正读取手机存储。<br /><br />
          点「关闭 SoloMD」→ 从后台任务里划掉 → 重新打开 App，再点「打开目录库」就能选你的笔记文件夹了。
        </p>
        <div class="arn__foot">
          <button class="arn__btn" @click="androidRestartNeeded = false">稍后</button>
          <button class="arn__btn arn__btn--primary" @click="doAndroidRestart">关闭 SoloMD</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mobile-app {
  /* Android edge-to-edge (MainActivity.enableEdgeToEdge) draws the WebView
     from y=0, i.e. *under* the system status bar. Reserve that height here so
     the status bar, the app top bar and the drawer never overlap. env() is the
     real inset on WebViews that report it; 40px is the fallback on those that
     report 0 (one status-bar row). Both the top and bottom system bars are
     accounted for; box-sizing:border-box keeps 100dvh from overflowing. */
  --safe-top: max(env(safe-area-inset-top, 0px), 40px);
  --safe-bottom: env(safe-area-inset-bottom, 0px);
  box-sizing: border-box;
  width: 100vw;
  height: 100vh;
  height: 100dvh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding-top: var(--safe-top);
  padding-bottom: var(--safe-bottom);
  background: #1a1a1a;
  color: #ffffff;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  position: relative;
  user-select: none;
}

/* ── Phone Status Bar (Markor-style) ──────────────────────────── */
/* Simulates the Android system status bar: dark, minimal, with time
   on the left and signal/wifi/battery on the right. This creates the
   visual offset that aligns the app bar with the reference layout. */
.phone-statusbar {
  /* 真机已有系统状态栏；edge-to-edge 下再画一条模拟栏只会重复显示时间 /
     信号 / 电量，也容易与系统栏重叠。整条隐藏，只保留系统状态栏；DOM 与
     计时逻辑保留，便于日后需要时恢复。 */
  display: none !important;
  height: 28px;
  min-height: 28px;
  align-items: center;
  justify-content: space-between;
  padding: 0 12px;
  background: #1a1a1a;
  color: #ffffff;
  font-size: 13px;
  font-weight: 500;
  flex-shrink: 0;
  /* Positioned so its z-index reliably wins over the fixed drawer (49). */
  position: relative;
  z-index: 60;
  user-select: none;
}

.phone-statusbar__time {
  letter-spacing: 0.5px;
  font-variant-numeric: tabular-nums;
}

.phone-statusbar__right {
  display: flex;
  align-items: center;
  gap: 6px;
}

.phone-statusbar__icon {
  opacity: 0.8;
}

.phone-statusbar__signal {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.3px;
}

.phone-statusbar__wifi {
  opacity: 0.7;
}

.phone-statusbar__battery {
  display: flex;
  align-items: center;
  gap: 3px;
}

.phone-statusbar__battery-body {
  width: 22px;
  height: 11px;
  border: 1.2px solid rgba(255, 255, 255, 0.6);
  border-radius: 3px;
  padding: 1px;
  position: relative;
}

.phone-statusbar__battery-fill {
  width: 79%;
  height: 100%;
  background: #ffffff;
  border-radius: 1px;
}

.phone-statusbar__battery-cap {
  width: 2px;
  height: 5px;
  background: rgba(255, 255, 255, 0.6);
  border-radius: 0 1px 1px 0;
  margin-left: -1px;
}

.phone-statusbar__battery-text {
  font-size: 11px;
  font-weight: 500;
  margin-left: 2px;
}

/* ── Mobile Topbar (Markor-style) ─────────────────────────────── */
/* Dark, minimal app bar — filename + unsaved dot, aligned with the
   reference page's upper bar position. */
.mobile-topbar {
  height: 44px;
  min-height: 44px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 6px 0 8px;
  background: #1a1a1a;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  /* Positioned so its z-index reliably wins over the fixed drawer (49). */
  position: relative;
  z-index: 50;
  user-select: none;
  color: #ffffff;
}

.mobile-topbar__left {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex: 1;
}

.mobile-topbar__right {
  display: flex;
  align-items: center;
  gap: 2px;
  flex-shrink: 0;
}

.topbar-title-wrap {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  overflow: hidden;
}

.topbar-title {
  font-size: 15px;
  font-weight: 600;
  color: #ffffff;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.topbar-dirty-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #ff9f40;
  flex-shrink: 0;
}

.topbar-btn {
  width: 40px;
  height: 40px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: var(--r-md, 8px);
  background: transparent;
  color: #ffffff;
  cursor: pointer;
  touch-action: manipulation;
  transition: background 0.15s ease;
  opacity: 0.85;
}

.topbar-btn:active {
  background: rgba(255, 255, 255, 0.12);
  opacity: 1;
}

.topbar-btn--active {
  background: rgba(255, 159, 64, 0.18);
  color: #ff9f40;
  opacity: 1;
}

/* More Menu */
.more-menu-wrapper {
  position: relative;
}

.more-menu-scrim {
  position: fixed;
  inset: 0;
  z-index: 99;
}

.more-menu-dropdown {
  position: absolute;
  top: 44px;
  right: 4px;
  min-width: 160px;
  background: #2a2a2a;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: var(--r-md, 10px);
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.4);
  padding: 6px;
  z-index: 100;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.more-menu-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  height: 40px;
  padding: 0 12px;
  border: 0;
  border-radius: var(--r-sm, 6px);
  background: transparent;
  color: #ffffff;
  font-size: 14px;
  cursor: pointer;
  touch-action: manipulation;
}

.more-menu-item:active {
  background: rgba(255, 255, 255, 0.1);
}

/* Drawer */
.drawer-scrim {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  /* 层级叠加：置于状态栏/顶栏之下，顶部栏不被遮罩盖住 */
  z-index: 48;
  backdrop-filter: blur(2px);
}

.drawer-fade-enter-active,
.drawer-fade-leave-active {
  transition: opacity 0.25s ease;
}
.drawer-fade-enter-from,
.drawer-fade-leave-to {
  opacity: 0;
}

.drawer-sidebar {
  position: fixed;
  /* 顶部三段互不重叠（Android edge-to-edge 下 WebView 从 y=0 绘制）：
       [0, safe-top)                 -> 系统状态栏，抽屉不进入
       [safe-top, safe-top + 44px)  -> 应用顶栏，抽屉不进入
       [safe-top + 44px, bottom)    -> 抽屉本体
     所以抽屉从顶栏下沿开始，既避开系统状态栏，也不与顶栏叠压；
     padding-top 归零，抽屉顶部不再有空白块。 */
  top: calc(var(--safe-top, 40px) + 44px);
  bottom: var(--safe-bottom, 0px);
  left: 0;
  width: 300px;
  max-width: 84vw;
  background: #1a1a1a;
  /* 高于遮罩(48)、低于顶栏(50)：顶栏始终可见且可点击关闭抽屉 */
  z-index: 49;
  display: flex;
  flex-direction: column;
  /* 抽屉整体可下拉滚动，滚动不冒泡到页面 */
  overflow-y: auto;
  overflow-x: hidden;
  -webkit-overflow-scrolling: touch;
  overscroll-behavior: contain;
  touch-action: pan-y;
  box-shadow: 2px 0 20px rgba(0, 0, 0, 0.4);
  transform: translateX(-100%);
  transition: transform 0.25s cubic-bezier(0.16, 1, 0.3, 1);
  color: #ffffff;
}

.drawer-sidebar--open {
  transform: translateX(0);
}

.drawer-header {
  height: 52px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  flex-shrink: 0;
}

.drawer-vault-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex: 1;
  overflow: hidden;
}

.drawer-vault-icon {
  color: #ff9f40;
  flex-shrink: 0;
}

.drawer-vault-name {
  font-size: 14px;
  font-weight: 600;
  color: #ffffff;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.drawer-switch-btn {
  height: 30px;
  padding: 0 10px;
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: var(--r-sm, 6px);
  background: transparent;
  color: #ffffff;
  font-size: 12px;
  cursor: pointer;
  flex-shrink: 0;
}

.drawer-switch-btn:active {
  background: rgba(255, 255, 255, 0.1);
}

.drawer-tree-container {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  -webkit-overflow-scrolling: touch;
  overscroll-behavior: contain;
  touch-action: pan-y;
}

/* Main Viewport */
.main-viewport {
  flex: 1;
  min-height: 0;
  position: relative;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.empty-viewport {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: #888;
}

.empty-icon {
  opacity: 0.35;
}

.empty-text {
  font-size: 15px;
  margin: 0;
}

.empty-action-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 40px;
  padding: 0 18px;
  border: 0;
  border-radius: var(--r-md, 8px);
  background: #ff9f40;
  color: #1a1a1a;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
}

/* Search Modal Overlay */
.search-modal-scrim {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  /* 弹窗也避开系统状态栏 / 导航栏，避免顶部输入框被压住 */
  padding: calc(var(--safe-top, 40px) + 12px) 12px calc(var(--safe-bottom, 0px) + 12px);
}

.search-modal-box {
  width: 100%;
  max-width: 540px;
  max-height: 85vh;
  background: #1a1a1a;
  border-radius: var(--r-lg, 12px);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
  color: #ffffff;
}

/* Android restart prompt */
.arn-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  z-index: 2000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: calc(var(--safe-top, 40px) + 16px) 16px calc(var(--safe-bottom, 0px) + 16px);
}
.arn {
  background: #1a1a1a;
  color: #ffffff;
  border-radius: var(--r-lg, 12px);
  width: 100%;
  max-width: 380px;
  padding: 20px;
  box-shadow: 0 8px 28px rgba(0, 0, 0, 0.4);
}
.arn__title {
  font-size: 16px;
  font-weight: 600;
  margin-bottom: 12px;
}
.arn__body {
  font-size: 14px;
  line-height: 1.6;
  color: #a9a6a0;
  margin: 0 0 16px;
}
.arn__foot {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.arn__btn {
  height: 36px;
  padding: 0 14px;
  border-radius: var(--r-md, 6px);
  border: 1px solid rgba(255, 255, 255, 0.15);
  background: transparent;
  color: #ffffff;
  font-size: 13px;
  cursor: pointer;
}
.arn__btn--primary {
  background: #ff9f40;
  border-color: #ff9f40;
  color: #1a1a1a;
}
</style>

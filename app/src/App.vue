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
  window.removeEventListener('solomd:file-opened', onFileOpened);
  window.removeEventListener('blur', onWindowBlur);
});
</script>

<template>
  <div class="mobile-app" :data-theme="dataThemeFor(settings.theme, !!settings.customCssPath)">
    <!-- 1. Mobile Top Bar -->
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
  width: 100vw;
  height: 100vh;
  height: 100dvh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--bg, #ffffff);
  color: var(--text, #111111);
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  position: relative;
  user-select: none;
}

/* Mobile Topbar */
.mobile-topbar {
  height: 48px;
  min-height: 48px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 6px 0 8px;
  background: var(--bg-sidebar, var(--bg));
  border-bottom: var(--bd-hair, 1px solid rgba(0, 0, 0, 0.08));
  z-index: 50;
  user-select: none;
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
  color: var(--text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.topbar-dirty-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--accent, #ff9f40);
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
  color: var(--text);
  cursor: pointer;
  touch-action: manipulation;
  transition: background 0.15s ease;
}

.topbar-btn:active {
  background: var(--fill-2, rgba(0, 0, 0, 0.08));
}

.topbar-btn--active {
  background: var(--accent-soft, rgba(255, 159, 64, 0.15));
  color: var(--accent-text, #ff9f40);
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
  background: var(--bg-elevated, var(--bg));
  border: 1px solid var(--border, rgba(0, 0, 0, 0.15));
  border-radius: var(--r-md, 10px);
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.15);
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
  color: var(--text);
  font-size: 14px;
  cursor: pointer;
  touch-action: manipulation;
}

.more-menu-item:active {
  background: var(--fill-2, rgba(0, 0, 0, 0.06));
}

/* Drawer */
.drawer-scrim {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  z-index: 100;
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
  top: 0;
  bottom: 0;
  left: 0;
  width: 300px;
  max-width: 84vw;
  background: var(--bg-sidebar, var(--bg));
  z-index: 101;
  display: flex;
  flex-direction: column;
  box-shadow: 2px 0 20px rgba(0, 0, 0, 0.18);
  transform: translateX(-100%);
  transition: transform 0.25s cubic-bezier(0.16, 1, 0.3, 1);
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
  border-bottom: var(--bd-hair, 1px solid rgba(0, 0, 0, 0.08));
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
  color: var(--accent, #ff9f40);
  flex-shrink: 0;
}

.drawer-vault-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.drawer-switch-btn {
  height: 30px;
  padding: 0 10px;
  border: 1px solid var(--border, rgba(0, 0, 0, 0.15));
  border-radius: var(--r-sm, 6px);
  background: var(--bg);
  color: var(--text);
  font-size: 12px;
  cursor: pointer;
  flex-shrink: 0;
}

.drawer-switch-btn:active {
  background: var(--fill-2, rgba(0, 0, 0, 0.08));
}

.drawer-tree-container {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
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
  color: var(--text-muted, #777);
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
  background: var(--accent, #ff9f40);
  color: var(--accent-fg, #fff);
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
}

/* Search Modal Overlay */
.search-modal-scrim {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 12px;
}

.search-modal-box {
  width: 100%;
  max-width: 540px;
  max-height: 85vh;
  background: var(--bg);
  border-radius: var(--r-lg, 12px);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.25);
}

/* Android restart prompt */
.arn-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  z-index: 2000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
}
.arn {
  background: var(--bg, #fff);
  color: var(--text, #111);
  border-radius: var(--r-lg, 12px);
  width: 100%;
  max-width: 380px;
  padding: 20px;
  box-shadow: 0 8px 28px rgba(0, 0, 0, 0.2);
}
.arn__title {
  font-size: 16px;
  font-weight: 600;
  margin-bottom: 12px;
}
.arn__body {
  font-size: 14px;
  line-height: 1.6;
  color: var(--text-2, #555);
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
  border: 1px solid var(--border, rgba(0, 0, 0, 0.15));
  background: transparent;
  color: var(--text, #111);
  font-size: 13px;
  cursor: pointer;
}
.arn__btn--primary {
  background: var(--accent, #ff9f40);
  border-color: var(--accent, #ff9f40);
  color: var(--accent-fg, #fff);
}
</style>

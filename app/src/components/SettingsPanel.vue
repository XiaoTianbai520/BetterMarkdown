<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { getVersion } from '@tauri-apps/api/app';
import { openUrl } from '@tauri-apps/plugin-opener';
import { useSettingsStore } from '../stores/settings';
import { useI18n } from '../i18n';
import { DsModal } from '../ui';
import Icons from './Icons.vue';
import BrandMark from './BrandMark.vue';

type SettingsCategory = 'editor' | 'appearance' | 'language' | 'about';

defineProps<{
  open: boolean;
  initialSection?: string | null;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const settings = useSettingsStore();
const { t } = useI18n();

const activeCategory = ref<SettingsCategory>('editor');

const categories = computed<{ id: SettingsCategory; icon: string; label: string }[]>(() => [
  { id: 'editor', icon: 'pencil', label: t('settings.catWriting') || '编辑器偏好' },
  { id: 'appearance', icon: 'theme-light', label: t('settings.appearance') || '外观主题' },
  { id: 'language', icon: 'globe', label: t('settings.language') || '语言' },
  { id: 'about', icon: 'info', label: t('menu.help.about') || '关于' },
]);

const version = ref('2.5.0');

onMounted(async () => {
  try {
    version.value = await getVersion();
  } catch {
    version.value = '2.5.0';
  }
});

function openGithub() {
  openUrl('https://github.com/solomd-app/solomd').catch(() => {});
}
</script>

<template>
  <DsModal
    :model-value="open"
    :title="t('settings.title') || '设置'"
    width="min(540px, 94vw)"
    @update:model-value="emit('close')"
  >
    <div class="settings-container">
      <!-- Category Tabs (Touch-friendly segmented control) -->
      <nav class="settings-tabs" role="tablist">
        <button
          v-for="cat in categories"
          :key="cat.id"
          class="settings-tab-btn"
          :class="{ 'settings-tab-btn--active': activeCategory === cat.id }"
          role="tab"
          :aria-selected="activeCategory === cat.id"
          @click="activeCategory = cat.id"
        >
          <Icons :name="cat.icon" :size="16" />
          <span>{{ cat.label }}</span>
        </button>
      </nav>

      <!-- Category Content -->
      <div class="settings-content">
        <!-- 1. Editor Preferences -->
        <section v-if="activeCategory === 'editor'" class="settings-section">
          <!-- Font Size -->
          <div class="setting-item">
            <div class="setting-item__header">
              <span class="setting-item__label">{{ t('settings.fontSize') || '字体大小' }}</span>
              <span class="setting-item__value">{{ settings.fontSize }}px</span>
            </div>
            <input
              type="range"
              min="12"
              max="28"
              class="setting-slider"
              :value="settings.fontSize"
              @input="settings.setFontSize(+($event.target as HTMLInputElement).value)"
            />
          </div>

          <!-- Line Height -->
          <div class="setting-item">
            <div class="setting-item__header">
              <span class="setting-item__label">行高</span>
              <span class="setting-item__value">{{ settings.lineHeight }}</span>
            </div>
            <input
              type="range"
              min="1.2"
              max="2.4"
              step="0.1"
              class="setting-slider"
              :value="settings.lineHeight"
              @input="settings.setLineHeight(+($event.target as HTMLInputElement).value)"
            />
          </div>

          <!-- Word Wrap -->
          <div class="setting-item setting-item--inline">
            <span class="setting-item__label">{{ t('settings.wordWrap') || '自动换行' }}</span>
            <label class="toggle-switch">
              <input
                type="checkbox"
                :checked="settings.wordWrap"
                @change="settings.toggleWordWrap()"
              />
              <span class="toggle-slider" />
            </label>
          </div>

          <!-- Auto-save Interval -->
          <div class="setting-item">
            <div class="setting-item__header">
              <span class="setting-item__label">自动保存间隔</span>
            </div>
            <select
              class="setting-select"
              :value="settings.autoSaveInterval"
              @change="settings.setAutoSaveInterval(+($event.target as HTMLSelectElement).value)"
            >
              <option :value="0">关闭</option>
              <option :value="5">5 秒</option>
              <option :value="10">10 秒</option>
              <option :value="30">30 秒</option>
              <option :value="60">1 分钟</option>
            </select>
          </div>
        </section>

        <!-- 2. Appearance Theme -->
        <section v-else-if="activeCategory === 'appearance'" class="settings-section">
          <!-- Follow System -->
          <div class="setting-item setting-item--inline">
            <div>
              <div class="setting-item__label">{{ t('settings.followSystemTheme') || '跟随系统主题' }}</div>
              <div class="setting-item__desc">根据 Android 系统浅色/深色模式自动切换</div>
            </div>
            <label class="toggle-switch">
              <input
                type="checkbox"
                :checked="settings.followSystemTheme"
                @change="settings.setTheme(settings.followSystemTheme ? settings.theme : 'system')"
              />
              <span class="toggle-slider" />
            </label>
          </div>

          <!-- Manual Theme Selection -->
          <div class="setting-item">
            <div class="setting-item__label" style="margin-bottom: 8px;">主题模式</div>
            <div class="theme-button-group">
              <button
                type="button"
                class="theme-btn"
                :class="{ 'theme-btn--active': !settings.followSystemTheme && settings.theme === 'light' }"
                :disabled="settings.followSystemTheme"
                @click="settings.setTheme('light')"
              >
                <Icons name="theme-light" :size="18" />
                <span>浅色</span>
              </button>
              <button
                type="button"
                class="theme-btn"
                :class="{ 'theme-btn--active': !settings.followSystemTheme && settings.theme === 'dark' }"
                :disabled="settings.followSystemTheme"
                @click="settings.setTheme('dark')"
              >
                <Icons name="theme-dark" :size="18" />
                <span>深色</span>
              </button>
            </div>
          </div>
        </section>

        <!-- 3. Language -->
        <section v-else-if="activeCategory === 'language'" class="settings-section">
          <div class="setting-item">
            <div class="setting-item__label" style="margin-bottom: 10px;">{{ t('settings.language') || '界面语言' }}</div>
            <div class="language-options">
              <label class="radio-option">
                <input
                  type="radio"
                  name="app_lang"
                  value="auto"
                  :checked="settings.language === 'auto'"
                  @change="settings.setLanguage('auto')"
                />
                <span class="radio-label">跟随系统 (System Default)</span>
              </label>
              <label class="radio-option">
                <input
                  type="radio"
                  name="app_lang"
                  value="zh"
                  :checked="settings.language === 'zh'"
                  @change="settings.setLanguage('zh')"
                />
                <span class="radio-label">简体中文</span>
              </label>
              <label class="radio-option">
                <input
                  type="radio"
                  name="app_lang"
                  value="en"
                  :checked="settings.language === 'en'"
                  @change="settings.setLanguage('en')"
                />
                <span class="radio-label">English</span>
              </label>
            </div>
          </div>
        </section>

        <!-- 4. About & Version -->
        <section v-else-if="activeCategory === 'about'" class="settings-section about-section">
          <div class="about-logo">
            <BrandMark :size="48" />
          </div>
          <h2 class="about-title">SoloMD</h2>
          <div class="about-version">版本 v{{ version }} (Android 专属轻量版)</div>
          <p class="about-desc">
            触控优先的 Android 单文档 Markdown 编辑器。<br />
            敏捷、极简、纯粹，专注于流畅的书写与阅读体验。
          </p>
          <div class="about-meta">
            <div class="about-meta-row">
              <span class="meta-label">渲染引擎</span>
              <span class="meta-val">CodeMirror 6 + KaTeX</span>
            </div>
            <div class="about-meta-row">
              <span class="meta-label">存储适配</span>
              <span class="meta-val">Android SAF & 本地文件系统</span>
            </div>
          </div>
          <button type="button" class="about-link-btn" @click="openGithub">
            <Icons name="globe" :size="16" />
            <span>GitHub 仓库</span>
          </button>
        </section>
      </div>
    </div>
  </DsModal>
</template>

<style scoped>
.settings-container {
  display: flex;
  flex-direction: column;
  gap: 16px;
  max-height: 75vh;
  min-height: 320px;
}

.settings-tabs {
  display: flex;
  background: var(--fill-2, rgba(0, 0, 0, 0.05));
  border-radius: var(--r-md, 8px);
  padding: 3px;
  overflow-x: auto;
  scrollbar-width: none;
}
.settings-tabs::-webkit-scrollbar {
  display: none;
}

.settings-tab-btn {
  flex: 1;
  min-width: 80px;
  height: 38px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border: 0;
  border-radius: var(--r-sm, 6px);
  background: transparent;
  color: var(--text-2, #666);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  white-space: nowrap;
  touch-action: manipulation;
  transition: all 0.15s ease;
}

.settings-tab-btn--active {
  background: var(--bg, #fff);
  color: var(--text, #111);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.08);
}

.settings-content {
  flex: 1;
  overflow-y: auto;
  padding: 4px 2px;
}

.settings-section {
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.setting-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.setting-item--inline {
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
}

.setting-item__header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.setting-item__label {
  font-size: 14px;
  font-weight: 500;
  color: var(--text, #111);
}

.setting-item__desc {
  font-size: 12px;
  color: var(--text-muted, #777);
  margin-top: 2px;
}

.setting-item__value {
  font-size: 13px;
  color: var(--text-muted, #777);
  font-variant-numeric: tabular-nums;
}

.setting-slider {
  width: 100%;
  height: 28px;
  cursor: pointer;
  accent-color: var(--accent, #ff9f40);
}

.setting-select {
  height: 40px;
  padding: 0 12px;
  border-radius: var(--r-md, 8px);
  border: 1px solid var(--border, rgba(0, 0, 0, 0.15));
  background: var(--bg-card, var(--bg, #fff));
  color: var(--text, #111);
  font-size: 14px;
  outline: none;
}

/* Toggle switch */
.toggle-switch {
  position: relative;
  display: inline-block;
  width: 44px;
  height: 24px;
}
.toggle-switch input {
  opacity: 0;
  width: 0;
  height: 0;
}
.toggle-slider {
  position: absolute;
  cursor: pointer;
  inset: 0;
  background-color: var(--fill-3, #ccc);
  transition: 0.2s;
  border-radius: 24px;
}
.toggle-slider:before {
  position: absolute;
  content: "";
  height: 18px;
  width: 18px;
  left: 3px;
  bottom: 3px;
  background-color: white;
  transition: 0.2s;
  border-radius: 50%;
  box-shadow: 0 1px 3px rgba(0,0,0,0.2);
}
input:checked + .toggle-slider {
  background-color: var(--accent, #ff9f40);
}
input:checked + .toggle-slider:before {
  transform: translateX(20px);
}

/* Theme buttons */
.theme-button-group {
  display: flex;
  gap: 12px;
}
.theme-btn {
  flex: 1;
  height: 44px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  border: 1px solid var(--border, rgba(0, 0, 0, 0.15));
  border-radius: var(--r-md, 8px);
  background: var(--bg-card, var(--bg, #fff));
  color: var(--text, #111);
  font-size: 14px;
  cursor: pointer;
  transition: all 0.15s ease;
}
.theme-btn--active {
  border-color: var(--accent, #ff9f40);
  background: var(--accent-soft, rgba(255, 159, 64, 0.12));
  color: var(--accent-text, #ff9f40);
  font-weight: 500;
}
.theme-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

/* Radio options */
.language-options {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.radio-option {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-radius: var(--r-md, 8px);
  background: var(--fill-1, rgba(0, 0, 0, 0.03));
  cursor: pointer;
  user-select: none;
}
.radio-option input[type="radio"] {
  accent-color: var(--accent, #ff9f40);
  width: 18px;
  height: 18px;
}
.radio-label {
  font-size: 14px;
  color: var(--text, #111);
}

/* About */
.about-section {
  align-items: center;
  text-align: center;
  padding: 12px 8px;
}
.about-logo {
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 8px;
}
.about-title {
  font-size: 20px;
  font-weight: 700;
  margin: 0;
  color: var(--text, #111);
}
.about-version {
  font-size: 12px;
  color: var(--text-muted, #777);
  margin-top: 4px;
}
.about-desc {
  font-size: 13px;
  line-height: 1.6;
  color: var(--text-2, #555);
  margin: 12px 0 16px;
  max-width: 380px;
}
.about-meta {
  width: 100%;
  max-width: 380px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  background: var(--fill-1, rgba(0, 0, 0, 0.03));
  border-radius: var(--r-md, 8px);
  margin-bottom: 16px;
}
.about-meta-row {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
}
.meta-label {
  color: var(--text-muted, #777);
}
.meta-val {
  color: var(--text, #111);
  font-weight: 500;
}
.about-link-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 36px;
  padding: 0 16px;
  border-radius: var(--r-full, 999px);
  border: 1px solid var(--border, rgba(0, 0, 0, 0.15));
  background: transparent;
  color: var(--text, #111);
  font-size: 13px;
  cursor: pointer;
  touch-action: manipulation;
}
</style>

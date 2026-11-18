import { createApp } from 'vue';
import { createPinia } from 'pinia';
import App from './App.vue';
import { followHtmlLang } from './i18n';
import { preloadRenderDeps } from './lib/render-deps';
import './styles/cjk-font.css';
import './styles/main.css';
import './styles/hljs-theme.css';
import 'katex/dist/katex.min.css';

const app = createApp(App);
app.use(createPinia());
followHtmlLang();
app.mount('#app');

// KaTeX + highlight.js are their own chunks (lib/render-deps.ts): fetch and
// compile them once the first frame is up, so they are almost always ready
// before a formula or code fence is on screen.
preloadRenderDeps();

// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a dedicated SPA fallback page.
// Keep it different from index.html to avoid adapter-static overwriting the main entry during build.
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

const config: import("@sveltejs/kit").Config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      fallback: "200.html"
    })
  }
};

export default config;

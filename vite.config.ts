import { paraglideVitePlugin } from "@inlang/paraglide-js";
import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const file = fileURLToPath(new URL("package.json", import.meta.url));
const pkg = JSON.parse(readFileSync(file, "utf8"));

const host = process.env.TAURI_DEV_HOST || "localhost";

export default defineConfig({
  plugins: [
    sveltekit(),
    tailwindcss(),
    paraglideVitePlugin({
      project: "./src/lib/i18n/project.inlang",
      outdir: "./src/lib/i18n/paraglide",
      strategy: ["localStorage", "preferredLanguage", "url", "baseLocale"]
    })
  ],
  root: process.cwd(),
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421, overlay: false } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
    open: false
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  define: {
    __APP_PKG__: pkg
  }
});

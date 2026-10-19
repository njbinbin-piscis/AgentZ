import { readFileSync } from "node:fs";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;
const pkg = JSON.parse(
  readFileSync(new URL("./package.json", import.meta.url), "utf-8"),
) as { version: string };

export default defineConfig({
  // Relative asset paths so a future Tauri desktop build can load bundled
  // files regardless of how the app protocol/root resolves at runtime.
  base: "./",
  plugins: [react()],
  clearScreen: false,
  // Expose the app version so the title bar can show which build is running.
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  build: {
    // Keep the production bundle compressed; the current chunk graph passes
    // Vite import analysis with esbuild minification enabled.
    minify: "esbuild",
    rollupOptions: {
      output: {
        manualChunks(id) {
          const norm = id.replace(/\\/g, "/");
          if (norm.includes("commonjsHelpers")) {
            return "app-vendor";
          }
          if (!norm.includes("/node_modules/")) {
            return undefined;
          }
          if (
            norm.includes("/node_modules/react/") ||
            norm.includes("/node_modules/react-dom/") ||
            norm.includes("/node_modules/scheduler/") ||
            norm.includes("/node_modules/use-sync-external-store/") ||
            norm.includes("/node_modules/i18next/") ||
            norm.includes("/node_modules/react-i18next/") ||
            norm.includes("/node_modules/@tauri-apps/")
          ) {
            return "app-vendor";
          }
          if (
            norm.includes("/node_modules/monaco-editor/") ||
            norm.includes("/node_modules/@monaco-editor/")
          ) {
            return "monaco";
          }
          if (norm.includes("/node_modules/@xterm/")) {
            return "xterm";
          }
          if (
            norm.includes("/node_modules/react-markdown/") ||
            norm.includes("/node_modules/remark-") ||
            norm.includes("/node_modules/rehype-") ||
            norm.includes("/node_modules/unified/") ||
            norm.includes("/node_modules/mdast-") ||
            norm.includes("/node_modules/hast-") ||
            norm.includes("/node_modules/micromark") ||
            norm.includes("/node_modules/vfile") ||
            norm.includes("/node_modules/highlight.js/") ||
            norm.includes("/node_modules/lowlight/") ||
            norm.includes("/node_modules/katex/")
          ) {
            return "markdown";
          }
          if (
            norm.includes("/node_modules/mermaid/") ||
            norm.includes("/node_modules/elkjs/") ||
            norm.includes("/node_modules/reactflow/") ||
            norm.includes("/node_modules/@reactflow/")
          ) {
            return "mermaid";
          }
          return undefined;
        },
      },
    },
  },
  server: {
    port: 5273,
    strictPort: true,
    host: host || false,
    watch: {
      ignored: [
        "**/src-tauri/**",
        "**/target/**",
        "**/.cache/**",
        "**/bundled/preinstall/**",
        "**/node_modules/**",
      ],
    },
  },
  optimizeDeps: { entries: ["src/main.tsx"] },
  test: {
    // scripts/*.test.mjs use node:test and run through `npm run test:scripts`.
    exclude: [
      "**/node_modules/**",
      "**/dist/**",
      "**/.{idea,git,cache,output,temp}/**",
      "scripts/**",
    ],
  },
});

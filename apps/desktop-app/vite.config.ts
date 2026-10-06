import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";

const ENVIRONMENTS = ["development", "staging", "production"];

// https://v2.tauri.app/start/frontend/vite/
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");

  if (!ENVIRONMENTS.includes(env.ENVIRONMENT ?? "")) {
    throw new Error(
      `ENVIRONMENT must be one of [${ENVIRONMENTS.join(",")}], found ${env.ENVIRONMENT}.`,
    );
  }

  return {
    plugins: [react()],
    build: { sourcemap: true },
    // Vite options tailored for Tauri development, applied in `tauri dev` or `tauri build`:
    // 1. do not obscure Rust errors
    clearScreen: false,
    // 2. Tauri expects a fixed port; fail if it is taken
    server: {
      port: 1420,
      strictPort: true,
      // 3. do not watch the Rust side
      watch: { ignored: ["**/backend/**", "**/target/**"] },
    },
    define: {
      __APP_VERSION__: JSON.stringify(env.npm_package_version),
      __ENVIRONMENT__: JSON.stringify(env.ENVIRONMENT),
      __AUTH_API_URL__: JSON.stringify(env.AUTH_API_URL),
    },
  };
});

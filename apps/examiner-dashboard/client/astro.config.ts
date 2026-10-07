import { defineConfig } from "astro/config";
import react from "@astrojs/react";

// Built to static files the eDd server serves from WEB_DIR. Same origin in production, so
// the dev server proxies /api to a locally running server instead of configuring CORS.
export default defineConfig({
  output: "static",
  integrations: [react()],
  vite: {
    server: { proxy: { "/api": "http://127.0.0.1:13003" } },
  },
});

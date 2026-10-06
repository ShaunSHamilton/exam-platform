import { defineConfig, envField } from "astro/config";
import react from "@astrojs/react";

// Static MPA: every page prerenders to HTML; React islands hydrate where interactive.
export default defineConfig({
  output: "static",
  integrations: [react()],
  env: {
    schema: {
      PUBLIC_AUTH_API_URL: envField.string({
        context: "client",
        access: "public",
        url: true,
        default: "http://127.0.0.1:13001",
      }),
    },
  },
});

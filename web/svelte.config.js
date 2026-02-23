import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import adapter from "@xevion/svelte-adapter-bun";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      out: "build",
      precompress: false,
      serveAssets: false,
    }),
  },
};

export default config;

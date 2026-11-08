import { defineConfig } from 'astro/config';
export default defineConfig({
  site: process.env.SITE_URL ?? 'https://huacnlee.github.io',
  base: process.env.BASE_PATH ?? '/spotish',
  trailingSlash: 'always',
  devToolbar: { enabled: false },
  // The site's own tsconfig, always: left to look, Vite also finds the app's one directory up, which extends
  // .nsh/tsconfig.json, a file `nsh check` generates and git ignores, so a fresh checkout (CI) fails to build.
  vite: { tsconfig: 'tsconfig.json' },
});

import { defineConfig, loadEnv } from 'vite';
import { fileURLToPath } from 'node:url';
import vue from '@vitejs/plugin-vue';
import i18n from 'laravel-vue-i18n/vite';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig(({ mode }) => {
  // Load env file based on `mode` in the current working directory.
  // Set the third parameter to '' to load all env regardless of the `VITE_` prefix.
  const env = loadEnv(mode, process.cwd(), '');
  // The Rust server, which `vite` (dev) forwards the API and DAV requests to.
  const server = env.MONICA_SERVER_URL || 'http://localhost:8000';
  const proxy = { target: server, changeOrigin: false, xfwd: true };
  return {
    plugins: [
      vue({
        template: {
          transformAssetUrls: {
            base: null,
            includeAbsolute: false,
          },
        },
      }),
      tailwindcss(),
      i18n(),
    ],
    resolve: {
      alias: { '@': fileURLToPath(new URL('./resources/js', import.meta.url)) },
    },
    server: {
      host: 'localhost',
      proxy: { '/api': proxy, '/dav': proxy, '/.well-known': proxy, '/up': proxy },
    },
    build: {
      outDir: 'dist',
      sourcemap: env.VITE_PROD_SOURCE_MAPS,
    },
  };
});

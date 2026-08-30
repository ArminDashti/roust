import path from 'node:path'
import { defineConfig, loadEnv } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '')
  const basePath = process.env.VITE_BASE_PATH || env.VITE_BASE_PATH || '/'
  const baseNoSlash = basePath.replace(/\/$/, '') || ''
  const apiTarget =
    process.env.VITE_API_PROXY_TARGET ||
    env.VITE_API_PROXY_TARGET ||
    'http://127.0.0.1:8787'
  const hmrClientPort = Number(
    process.env.VITE_HMR_CLIENT_PORT || env.VITE_HMR_CLIENT_PORT || 5173,
  )

  const apiProxyKey = baseNoSlash ? `${baseNoSlash}/api` : '/api'

  return {
    base: basePath,
    plugins: [vue(), tailwindcss()],
    resolve: {
      alias: {
        '@': path.resolve(__dirname, './src'),
      },
    },
    server: {
      host: true,
      port: 5173,
      strictPort: true,
      allowedHosts: true,
      hmr: { clientPort: hmrClientPort },
      proxy: {
        [apiProxyKey]: {
          target: apiTarget,
          changeOrigin: true,
          rewrite: (p) =>
            baseNoSlash
              ? p.replace(new RegExp(`^${baseNoSlash}/api`), '/api')
              : p,
        },
      },
    },
  }
})

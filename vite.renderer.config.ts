import { fileURLToPath } from 'node:url'
import { resolve } from 'node:path'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

const projectRoot = fileURLToPath(new URL('.', import.meta.url))

export default defineConfig({
  root: 'src/renderer',
  base: './',
  resolve: {
    alias: {
      '@': resolve(projectRoot, 'src/renderer/src'),
      '@shared': resolve(projectRoot, 'src/shared')
    }
  },
  plugins: [vue()],
  build: {
    outDir: resolve(projectRoot, 'dist/renderer'),
    emptyOutDir: true
  }
})

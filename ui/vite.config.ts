import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'node:path'

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': resolve(__dirname, './src'),
    },
  },
  // Production build configuration
  build: {
    // Output to dist directory
    outDir: 'dist',
    // Generate source maps for debugging
    sourcemap: false,
    // Minify with esbuild (default, fastest)
    minify: 'esbuild',
    // Chunk size warning threshold (500kb)
    chunkSizeWarningLimit: 500,
    // Code splitting configuration
    rollupOptions: {
      output: {
        // Manual chunk splitting for better caching
        manualChunks: {
          // Vue core
          'vue-vendor': ['vue'],
          // UI components
          'ui-vendor': ['radix-vue', 'reka-ui', 'class-variance-authority', 'clsx', 'tailwind-merge'],
          // Icons
          'icons': ['lucide-vue-next'],
        },
        // Asset file naming with hash for cache busting
        assetFileNames: 'assets/[name]-[hash][extname]',
        // Chunk file naming
        chunkFileNames: 'assets/[name]-[hash].js',
        // Entry file naming
        entryFileNames: 'assets/[name]-[hash].js',
      },
    },
    // Target modern browsers
    target: 'es2020',
  },
  // Development server configuration
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:3456',
        changeOrigin: true,
      },
    },
  },
})

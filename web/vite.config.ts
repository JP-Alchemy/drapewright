import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import { VitePWA } from 'vite-plugin-pwa'

// Cross-origin isolation from day one: enables SharedArrayBuffer and WASM
// threads later without changing how the app is served. Production hosts get
// the same headers from public/_headers.
const isolationHeaders = {
  'Cross-Origin-Opener-Policy': 'same-origin',
  'Cross-Origin-Embedder-Policy': 'require-corp',
}

export default defineConfig({
  plugins: [
    react(),
    VitePWA({
      registerType: 'autoUpdate',
      injectRegister: null,
      includeAssets: ['favicon.ico', 'apple-touch-icon-180x180.png', 'icon.svg'],
      manifest: {
        // The app is at /app/; the site's own pages are at the root. The id
        // stays as it was when the app was at the root, so an installed app
        // is the same app.
        id: '/',
        start_url: '/app/',
        scope: '/',
        name: 'Drapewright pattern studio',
        short_name: 'Drapewright',
        description: 'Open your sewing patterns, fit them, and print them at true size.',
        theme_color: '#2f5d50',
        background_color: '#efe9df',
        display: 'standalone',
        orientation: 'any',
        icons: [
          { src: 'pwa-64x64.png', sizes: '64x64', type: 'image/png' },
          { src: 'pwa-192x192.png', sizes: '192x192', type: 'image/png' },
          { src: 'pwa-512x512.png', sizes: '512x512', type: 'image/png' },
          {
            src: 'maskable-icon-512x512.png',
            sizes: '512x512',
            type: 'image/png',
            purpose: 'maskable',
          },
        ],
      },
      workbox: {
        globPatterns: ['**/*.{js,css,html,svg,png,jpg,webp,ico,wasm,webmanifest}'],
        // Offline, a page of the app opens the app; the site's pages are
        // kept as they are.
        navigateFallback: 'app/index.html',
        navigateFallbackAllowlist: [/^\/app(\/|$)/],
        maximumFileSizeToCacheInBytes: 16 * 1024 * 1024,
        // The avatar's body file (4 MB) is fetched the first time the
        // avatar is wanted, then kept for offline use. Its name changes with
        // its contents, so a cached copy never goes stale.
        runtimeCaching: [
          {
            urlPattern: ({ url }) => url.pathname.endsWith('.dwbody'),
            handler: 'CacheFirst',
            options: { cacheName: 'drapewright-body', expiration: { maxEntries: 2 } },
          },
        ],
      },
    }),
  ],
  server: { port: 5173, strictPort: true, headers: isolationHeaders },
  preview: { port: 4173, strictPort: true, headers: isolationHeaders },
  build: {
    target: 'es2022',
    // The site's front page, and the app.
    rollupOptions: { input: { home: 'index.html', app: 'app/index.html' } },
  },
  worker: { format: 'es' },
})

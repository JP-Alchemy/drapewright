import { execSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
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

/**
 * The headers the host sends with every page (public/_headers, one block
 * for every path). The preview, which the end-to-end tests run against,
 * sends them too, so the tests meet drapewright.com's own
 * Content-Security-Policy.
 */
function hostHeaders(): Record<string, string> {
  const file = readFileSync(new URL('./public/_headers', import.meta.url), 'utf8')
  return Object.fromEntries(
    file
      .split('\n')
      .filter((line) => /^\s+\S/.test(line))
      .map((line) => {
        const at = line.indexOf(':')
        return [line.slice(0, at).trim(), line.slice(at + 1).trim()]
      }),
  )
}

/** Which build this is, for feedback: the commit it was built from. */
function build(): string {
  if (process.env.GITHUB_SHA) return process.env.GITHUB_SHA.slice(0, 7)
  try {
    return execSync('git rev-parse --short HEAD', { stdio: ['ignore', 'pipe', 'ignore'] })
      .toString()
      .trim()
  } catch {
    return 'dev'
  }
}

export default defineConfig({
  define: {
    'import.meta.env.VITE_BUILD': JSON.stringify(build()),
  },
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
        // The samples (projects, and the dress as a PDF) and the dress's
        // licence too, to try offline.
        globPatterns: [
          '**/*.{js,css,html,svg,png,jpg,webp,ico,wasm,webmanifest,pdf,txt,drapewright}',
        ],
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
  preview: { port: 4173, strictPort: true, headers: hostHeaders() },
  build: {
    target: 'es2022',
    // The site's front page, and the app.
    rollupOptions: { input: { home: 'index.html', app: 'app/index.html' } },
  },
  worker: { format: 'es' },
})

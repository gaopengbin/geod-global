import tailwindcss from '@tailwindcss/vite';
import { cesiumAssets } from '../scripts/cesium-assets.mjs';

// Dependencies resolve only through this repository's root package and lockfile.
export default {
  plugins: [tailwindcss(), cesiumAssets()],
  define: { CESIUM_BASE_URL: JSON.stringify('cesium/') },
  base: "./",
  // Workspace-only imports need to be ready before its first lazy navigation.
  optimizeDeps: { include: ['ol/layer/Image.js', 'ol/source/ImageStatic.js', 'ol/geom/Point.js', 'ol/interaction/Draw.js'] },
  server: { host: "127.0.0.1", port: 4317, strictPort: true },
  preview: { host: "127.0.0.1", port: 4317, strictPort: true },
  build: { outDir: "dist", emptyOutDir: true },
};

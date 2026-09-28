import tailwindcss from '@tailwindcss/vite';

// Dependencies resolve only through this repository's root package and lockfile.
export default {
  plugins: [tailwindcss()],
  base: "./",
  server: { host: "127.0.0.1", port: 4317, strictPort: true },
  preview: { host: "127.0.0.1", port: 4317, strictPort: true },
  build: { outDir: "dist", emptyOutDir: true },
};

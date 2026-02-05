/// <reference types="vitest" />
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
var __dirname = dirname(fileURLToPath(import.meta.url));
export default defineConfig({
    plugins: [react(), tailwindcss()],
    resolve: {
        alias: {
            "@": resolve(__dirname, "./src"),
        },
    },
    server: {
        port: 5173,
        host: true, // Listen on all interfaces (0.0.0.0)
        allowedHosts: ["studio", "localhost"],
        proxy: {
            "/trpc": {
                target: "http://localhost:3000",
                changeOrigin: true,
            },
            "/ws": {
                target: "http://localhost:3000",
                ws: true,
                changeOrigin: true,
            },
        },
    },
    test: {
        globals: true,
        environment: "jsdom",
        setupFiles: ["./src/test/setup.ts"],
        include: ["src/**/*.{test,spec}.{ts,tsx}"],
    },
});

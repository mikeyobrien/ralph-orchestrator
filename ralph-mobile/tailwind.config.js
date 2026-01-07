/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./app/**/*.{js,jsx,ts,tsx}",
    "./components/**/*.{js,jsx,ts,tsx}",
  ],
  presets: [require("nativewind/preset")],
  theme: {
    extend: {
      colors: {
        slate: {
          900: "#0f172a",
          800: "#1e293b",
          700: "#334155",
          600: "#475569",
          500: "#64748b",
          400: "#94a3b8",
          300: "#cbd5e1",
        },
        indigo: {
          600: "#4f46e5",
          500: "#6366f1",
          400: "#818cf8",
        },
        emerald: {
          500: "#10b981",
          400: "#34d399",
        },
        amber: {
          500: "#f59e0b",
          400: "#fbbf24",
        },
        red: {
          500: "#ef4444",
          400: "#f87171",
        },
      },
    },
  },
  plugins: [],
};

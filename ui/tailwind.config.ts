import type { Config } from "tailwindcss";

const config: Config = {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  darkMode: ["class", '[data-theme="dark"]'],
  theme: {
    extend: {
      colors: {
        "bg-base": "var(--bg-base)",
        "bg-elev-1": "var(--bg-elev-1)",
        "bg-elev-2": "var(--bg-elev-2)",
        border: "var(--border)",
        "fg-primary": "var(--fg-primary)",
        "fg-secondary": "var(--fg-secondary)",
        "fg-tertiary": "var(--fg-tertiary)",
        accent: "var(--accent)",
        success: "var(--success)",
        warning: "var(--warning)",
        danger: "var(--danger)",
        critical: "var(--critical)",
        "success-bg": "var(--success-bg)",
        "success-border": "var(--success-border)",
        "warning-bg": "var(--warning-bg)",
        "warning-border": "var(--warning-border)",
        "danger-bg": "var(--danger-bg)",
        "danger-border": "var(--danger-border)",
        "info-bg": "var(--info-bg)",
        "info-border": "var(--info-border)",
        background: "var(--background)",
        foreground: "var(--foreground)",
        primary: "var(--primary)",
        "primary-foreground": "var(--primary-foreground)",
        muted: "var(--muted)",
        "muted-foreground": "var(--muted-foreground)",
        ring: "var(--ring)",
        card: "var(--card)",
        "card-foreground": "var(--card-foreground)",
        destructive: "var(--destructive)",
      },
      fontFamily: {
        sans: ["Inter", "Microsoft YaHei", "sans-serif"],
        mono: ["JetBrains Mono", "Cascadia Mono", "monospace"],
      },
      fontSize: {
        xs: ["11px", "16px"],
        sm: ["12px", "18px"],
        base: ["13px", "20px"],
        md: ["14px", "22px"],
        lg: ["16px", "24px"],
      },
    },
  },
  plugins: [],
};

export default config;

"use client";

import { useTheme } from "next-themes";
import { useState, useEffect } from "react";
import { SunIcon, MoonIcon, MonitorIcon } from "lucide-react";

const THEMES = [
  { value: "light", icon: SunIcon, label: "Light" },
  { value: "dark", icon: MoonIcon, label: "Dark" },
  { value: "system", icon: MonitorIcon, label: "System" },
] as const;

export function ThemeToggle() {
  const [mounted, setMounted] = useState(false);
  const { theme, setTheme } = useTheme();

  useEffect(() => setMounted(true), []);

  if (!mounted) {
    return <div className="h-8 w-[6.5rem] rounded-full bg-muted" />;
  }

  return (
    <div className="inline-flex items-center gap-0.5 rounded-full bg-muted p-0.5">
      {THEMES.map(({ value, icon: Icon, label }) => {
        const isActive = theme === value;
        return (
          <button
            key={value}
            type="button"
            onClick={() => setTheme(value)}
            aria-label={`Switch to ${label} theme`}
            className={`relative rounded-full p-1.5 transition-colors ${
              isActive
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground"
            }`}
          >
            <Icon className="size-3.5" />
          </button>
        );
      })}
    </div>
  );
}

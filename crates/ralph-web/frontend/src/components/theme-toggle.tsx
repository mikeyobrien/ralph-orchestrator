/**
 * Theme toggle button component
 */

import { Moon, Sun, Monitor } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useAppStore, type Theme } from '@/lib/store';

const icons: Record<Theme, React.ReactNode> = {
  light: <Sun className="h-4 w-4" />,
  dark: <Moon className="h-4 w-4" />,
  system: <Monitor className="h-4 w-4" />,
};

const nextTheme: Record<Theme, Theme> = {
  light: 'dark',
  dark: 'system',
  system: 'light',
};

export function ThemeToggle() {
  const theme = useAppStore((state) => state.theme);
  const setTheme = useAppStore((state) => state.setTheme);

  return (
    <Button
      variant="ghost"
      size="icon"
      onClick={() => setTheme(nextTheme[theme])}
      title={`Current theme: ${theme}. Click to switch.`}
      data-testid="theme-toggle"
    >
      {icons[theme]}
      <span className="sr-only">Toggle theme</span>
    </Button>
  );
}

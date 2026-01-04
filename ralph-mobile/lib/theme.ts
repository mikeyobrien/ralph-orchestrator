/**
 * Theme configuration for Ralph Mobile App
 * Dark theme matching the web UI aesthetic
 */

export const colors = {
  background: '#0a0a0a',
  surface: '#1a1a1a',
  primary: '#3b82f6',
  success: '#22c55e',
  error: '#ef4444',
  text: '#ffffff',
  textMuted: '#a1a1aa',
};

export const spacing = {
  xs: 4,
  sm: 8,
  md: 16,
  lg: 24,
  xl: 32,
};

export const typography = {
  sizes: {
    sm: 12,
    md: 14,
    lg: 16,
    xl: 20,
    xxl: 24,
  },
  weights: {
    normal: '400' as const,
    medium: '500' as const,
    bold: '700' as const,
  },
};

export const theme = {
  colors,
  spacing,
  typography,
};

export default theme;

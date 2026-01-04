/**
 * TDD Tests for Theme Configuration (Plan 04-02)
 * Following TDD: Write tests first, watch them fail, then implement
 */

import { colors, theme, spacing, typography } from '../lib/theme';

describe('Theme Configuration', () => {
  describe('colors', () => {
    it('has dark background color', () => {
      expect(colors.background).toBe('#0a0a0a');
    });

    it('has dark surface color', () => {
      expect(colors.surface).toBe('#1a1a1a');
    });

    it('has blue primary color', () => {
      expect(colors.primary).toBe('#3b82f6');
    });

    it('has green success color', () => {
      expect(colors.success).toBe('#22c55e');
    });

    it('has red error color', () => {
      expect(colors.error).toBe('#ef4444');
    });

    it('has white text color', () => {
      expect(colors.text).toBe('#ffffff');
    });

    it('has muted text color', () => {
      expect(colors.textMuted).toBe('#a1a1aa');
    });
  });

  describe('spacing', () => {
    it('has standard spacing values', () => {
      expect(spacing.xs).toBe(4);
      expect(spacing.sm).toBe(8);
      expect(spacing.md).toBe(16);
      expect(spacing.lg).toBe(24);
      expect(spacing.xl).toBe(32);
    });
  });

  describe('typography', () => {
    it('has font sizes', () => {
      expect(typography.sizes.sm).toBe(12);
      expect(typography.sizes.md).toBe(14);
      expect(typography.sizes.lg).toBe(16);
      expect(typography.sizes.xl).toBe(20);
      expect(typography.sizes.xxl).toBe(24);
    });

    it('has font weights', () => {
      expect(typography.weights.normal).toBe('400');
      expect(typography.weights.medium).toBe('500');
      expect(typography.weights.bold).toBe('700');
    });
  });

  describe('theme object', () => {
    it('exports complete theme', () => {
      expect(theme.colors).toEqual(colors);
      expect(theme.spacing).toEqual(spacing);
      expect(theme.typography).toEqual(typography);
    });
  });
});

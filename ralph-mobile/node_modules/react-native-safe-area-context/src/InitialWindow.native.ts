import type { Metrics } from './SafeArea.types';
import NativeSafeAreaContext from './specs/NativeSafeAreaContext';

export const initialWindowMetrics = (NativeSafeAreaContext?.getConstants?.()
  ?.initialWindowMetrics ?? null) as Metrics | null;

/**
 * @deprecated
 */
export const initialWindowSafeAreaInsets = initialWindowMetrics?.insets;

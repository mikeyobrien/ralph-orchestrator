import { jest } from '@jest/globals';
import React, { useContext } from 'react';
import type { Metrics } from '../src/SafeArea.types';
import type {
  SafeAreaProviderProps,
  SafeAreaInsetsContext,
  SafeAreaFrameContext,
} from '../src/SafeAreaContext';

const MOCK_INITIAL_METRICS: Metrics = {
  frame: {
    width: 320,
    height: 640,
    x: 0,
    y: 0,
  },
  insets: {
    left: 0,
    right: 0,
    bottom: 0,
    top: 0,
  },
};

const RNSafeAreaContext = jest.requireActual<{
  SafeAreaInsetsContext: typeof SafeAreaInsetsContext;
  SafeAreaFrameContext: typeof SafeAreaFrameContext;
}>('react-native-safe-area-context');

export default {
  ...RNSafeAreaContext,
  initialWindowMetrics: MOCK_INITIAL_METRICS,
  useSafeAreaInsets: () => {
    return (
      useContext(RNSafeAreaContext.SafeAreaInsetsContext) ??
      MOCK_INITIAL_METRICS.insets
    );
  },
  useSafeAreaFrame: () => {
    return (
      useContext(RNSafeAreaContext.SafeAreaFrameContext) ??
      MOCK_INITIAL_METRICS.frame
    );
  },
  // Provide a simpler implementation with default values.
  SafeAreaProvider: ({ children, initialMetrics }: SafeAreaProviderProps) => {
    return (
      <RNSafeAreaContext.SafeAreaFrameContext.Provider
        value={initialMetrics?.frame ?? MOCK_INITIAL_METRICS.frame}
      >
        <RNSafeAreaContext.SafeAreaInsetsContext.Provider
          value={initialMetrics?.insets ?? MOCK_INITIAL_METRICS.insets}
        >
          {children}
        </RNSafeAreaContext.SafeAreaInsetsContext.Provider>
      </RNSafeAreaContext.SafeAreaFrameContext.Provider>
    );
  },
};

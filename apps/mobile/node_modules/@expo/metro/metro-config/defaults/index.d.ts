// See: https://github.com/facebook/metro/blob/v0.83.2/packages/metro-config/src/defaults/index.js

// NOTE(cedric): This file can't be typed properly due to complex CJS structures
// NOTE(cedric): This file has lots more exports, but neither of them should be used directly by Expo

import type { ConfigT } from '../types';

interface getDefaultConfig {
  (rootPath: string | null): Promise<ConfigT>;
  getDefaultValues: (rootPath: string | null) => ConfigT;
}

declare const getDefaultValues: getDefaultConfig;
export default getDefaultValues;

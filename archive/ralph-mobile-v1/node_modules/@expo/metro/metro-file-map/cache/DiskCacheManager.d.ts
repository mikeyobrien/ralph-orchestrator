/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 *
 * @format
 * @oncall react_native
 */

import type { BuildParameters, CacheData, CacheManager, CacheManagerFactoryOptions, CacheManagerWriteOptions } from "../flow-types";
export interface AutoSaveOptions {
  readonly debounceMs: number;
}
export interface DiskCacheConfig {
  readonly autoSave?: Partial<AutoSaveOptions> | boolean;
  readonly cacheFilePrefix?: null | undefined | string;
  readonly cacheDirectory?: null | undefined | string;
}
export declare class DiskCacheManager implements CacheManager {
  constructor($$PARAM_0$$: CacheManagerFactoryOptions, $$PARAM_1$$: DiskCacheConfig);
  static getCacheFilePath(buildParameters: BuildParameters, cacheFilePrefix?: null | undefined | string, cacheDirectory?: null | undefined | string): string;
  getCacheFilePath(): string;
  read(): Promise<null | undefined | CacheData>;
  write(getSnapshot: () => CacheData, $$PARAM_1$$: CacheManagerWriteOptions): Promise<void>;
  end(): Promise<void>;
}
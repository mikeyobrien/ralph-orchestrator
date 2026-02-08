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

import type { Console, DuplicatesSet, FileMapDelta, FileMapPlugin, FileMapPluginInitOptions, FileMetadata, HasteConflict, HasteMap, HasteMapItemMetadata, HTypeValue, Path, PerfLogger } from "../flow-types";
export interface HasteMapOptions {
  readonly console?: null | undefined | Console;
  readonly enableHastePackages: boolean;
  readonly perfLogger?: null | PerfLogger;
  readonly platforms: ReadonlySet<string>;
  readonly rootDir: Path;
  readonly failValidationOnConflicts: boolean;
}
declare class HastePlugin implements HasteMap, FileMapPlugin<null> {
  readonly name: any;
  constructor(options: HasteMapOptions);
  initialize($$PARAM_0$$: FileMapPluginInitOptions<null>): Promise<void>;
  getSerializableSnapshot(): null;
  getModule(name: string, platform?: null | undefined | string, supportsNativePlatform?: null | undefined | boolean, type?: null | undefined | HTypeValue): null | undefined | Path;
  getPackage(name: string, platform: null | undefined | string, _supportsNativePlatform?: null | undefined | boolean): null | undefined | Path;
  _getModuleMetadata(name: string, platform: null | undefined | string, supportsNativePlatform: boolean): HasteMapItemMetadata | null;
  _assertNoDuplicates(name: string, platform: string, supportsNativePlatform: boolean, relativePathSet: null | undefined | DuplicatesSet): void;
  bulkUpdate(delta: FileMapDelta): Promise<void>;
  onNewOrModifiedFile(relativeFilePath: string, fileMetadata: FileMetadata): void;
  setModule(id: string, module: HasteMapItemMetadata): void;
  onRemovedFile(relativeFilePath: string, fileMetadata: FileMetadata): void;
  assertValid(): void;
  _recoverDuplicates(moduleName: string, relativeFilePath: string): void;
  computeConflicts(): Array<HasteConflict>;
  getCacheKey(): string;
}
export default HastePlugin;
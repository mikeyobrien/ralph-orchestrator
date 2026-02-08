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

import type { RootPerfLogger } from "../types";
export { default as defaultCreateModuleIdFactory } from "./createModuleIdFactory";
export declare const assetExts: Array<string>;
export declare const assetResolutions: Array<string>;
export declare const sourceExts: Array<string>;
export declare const additionalExts: Array<string>;
export declare const moduleSystem: string;
export declare const platforms: Array<string>;
export declare const DEFAULT_METRO_MINIFIER_PATH: "metro-minify-terser";
export declare const noopPerfLoggerFactory: () => RootPerfLogger;
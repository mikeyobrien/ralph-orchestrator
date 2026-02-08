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

/**
 * Raised when package imports do not define or permit a target subpath in the
 * package for the given import specifier.
 */
declare class PackageImportNotResolvedError extends Error {
  readonly importSpecifier: string;
  readonly reason: string;
  constructor(opts: {
    readonly importSpecifier: string;
    readonly reason: string;
  });
}
export default PackageImportNotResolvedError;
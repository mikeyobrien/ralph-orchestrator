/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<8dc575164f2fa359f14313300c2115ba>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/src/private/webapis/dom/nodes/ReadOnlyCharacterData.js
 */

import type ReadOnlyElement from "./ReadOnlyElement";
import ReadOnlyNode from "./ReadOnlyNode";
declare class ReadOnlyCharacterData extends ReadOnlyNode {
  get nextElementSibling(): ReadOnlyElement | null;
  get previousElementSibling(): ReadOnlyElement | null;
  get data(): string;
  get length(): number;
  /**
   * @override
   */
  get textContent(): string;
  /**
   * @override
   */
  get nodeValue(): string;
  substringData(offset: number, count: number): string;
}
export default ReadOnlyCharacterData;

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

import type { ModuleGroups, ModuleTransportLike } from "../../types";
import type { IndexMap } from "../../../../metro-source-map";
export interface Params {
  fixWrapperOffset: boolean;
  lazyModules: ReadonlyArray<ModuleTransportLike>;
  moduleGroups?: null | ModuleGroups;
  startupModules: ReadonlyArray<ModuleTransportLike>;
}
declare const $$EXPORT_DEFAULT_DECLARATION$$: ($$PARAM_0$$: Params) => IndexMap;
export default $$EXPORT_DEFAULT_DECLARATION$$;
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<c46c2ecc9012b9d05033191d208b0603>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/Libraries/Share/Share.js
 */

import type { ColorValue } from "../StyleSheet/StyleSheet";
export type ShareContent = {
  title?: string;
  url: string;
  message?: string;
} | {
  title?: string;
  url?: string;
  message: string;
};
export type ShareOptions = {
  dialogTitle?: string;
  excludedActivityTypes?: Array<string>;
  tintColor?: ColorValue;
  subject?: string;
  anchor?: number;
};
export type ShareAction = {
  action: "sharedAction" | "dismissedAction";
  activityType?: string | null;
};
declare class Share {
  /**
   * Open a dialog to share text content.
   *
   * In iOS, Returns a Promise which will be invoked an object containing `action`, `activityType`.
   * If the user dismissed the dialog, the Promise will still be resolved with action being `Share.dismissedAction`
   * and all the other keys being undefined.
   *
   * In Android, Returns a Promise which always resolves with action being `Share.sharedAction`.
   *
   * ### Content
   *
   * #### iOS
   *
   *  - `url` - a URL to share
   *  - `message` - a message to share
   *
   * At least one of `URL` or `message` is required.
   *
   * #### Android
   *
   * - `title` - title of the message (optional)
   * - `message` - a message to share (often will include a URL).
   *
   * ### Options
   *
   * #### iOS
   *
   *  - `subject` - a subject to share via email
   *  - `excludedActivityTypes`
   *  - `tintColor`
   *
   * #### Android
   *
   *  - `dialogTitle`
   *
   */
  static share(content: ShareContent, options?: ShareOptions): Promise<{
    action: string;
    activityType: string | undefined;
  }>;
  /**
   * The content was successfully shared.
   */
  static sharedAction: "sharedAction";
  /**
   * The dialog has been dismissed.
   * @platform ios
   */
  static dismissedAction: "dismissedAction";
}
export default Share;

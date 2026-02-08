/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<c49e0a6fcb9b42b838c5576c328d2525>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/Libraries/PermissionsAndroid/PermissionsAndroid.js
 */

export type Rationale = {
  title: string;
  message: string;
  buttonPositive?: string;
  buttonNegative?: string;
  buttonNeutral?: string;
};
type PermissionsType = Readonly<{
  READ_CALENDAR: "android.permission.READ_CALENDAR";
  WRITE_CALENDAR: "android.permission.WRITE_CALENDAR";
  CAMERA: "android.permission.CAMERA";
  READ_CONTACTS: "android.permission.READ_CONTACTS";
  WRITE_CONTACTS: "android.permission.WRITE_CONTACTS";
  GET_ACCOUNTS: "android.permission.GET_ACCOUNTS";
  ACCESS_FINE_LOCATION: "android.permission.ACCESS_FINE_LOCATION";
  ACCESS_COARSE_LOCATION: "android.permission.ACCESS_COARSE_LOCATION";
  ACCESS_BACKGROUND_LOCATION: "android.permission.ACCESS_BACKGROUND_LOCATION";
  RECORD_AUDIO: "android.permission.RECORD_AUDIO";
  READ_PHONE_STATE: "android.permission.READ_PHONE_STATE";
  CALL_PHONE: "android.permission.CALL_PHONE";
  READ_CALL_LOG: "android.permission.READ_CALL_LOG";
  WRITE_CALL_LOG: "android.permission.WRITE_CALL_LOG";
  ADD_VOICEMAIL: "com.android.voicemail.permission.ADD_VOICEMAIL";
  READ_VOICEMAIL: "com.android.voicemail.permission.READ_VOICEMAIL";
  WRITE_VOICEMAIL: "com.android.voicemail.permission.WRITE_VOICEMAIL";
  USE_SIP: "android.permission.USE_SIP";
  PROCESS_OUTGOING_CALLS: "android.permission.PROCESS_OUTGOING_CALLS";
  BODY_SENSORS: "android.permission.BODY_SENSORS";
  BODY_SENSORS_BACKGROUND: "android.permission.BODY_SENSORS_BACKGROUND";
  SEND_SMS: "android.permission.SEND_SMS";
  RECEIVE_SMS: "android.permission.RECEIVE_SMS";
  READ_SMS: "android.permission.READ_SMS";
  RECEIVE_WAP_PUSH: "android.permission.RECEIVE_WAP_PUSH";
  RECEIVE_MMS: "android.permission.RECEIVE_MMS";
  READ_EXTERNAL_STORAGE: "android.permission.READ_EXTERNAL_STORAGE";
  READ_MEDIA_IMAGES: "android.permission.READ_MEDIA_IMAGES";
  READ_MEDIA_VIDEO: "android.permission.READ_MEDIA_VIDEO";
  READ_MEDIA_AUDIO: "android.permission.READ_MEDIA_AUDIO";
  READ_MEDIA_VISUAL_USER_SELECTED: "android.permission.READ_MEDIA_VISUAL_USER_SELECTED";
  WRITE_EXTERNAL_STORAGE: "android.permission.WRITE_EXTERNAL_STORAGE";
  BLUETOOTH_CONNECT: "android.permission.BLUETOOTH_CONNECT";
  BLUETOOTH_SCAN: "android.permission.BLUETOOTH_SCAN";
  BLUETOOTH_ADVERTISE: "android.permission.BLUETOOTH_ADVERTISE";
  ACCESS_MEDIA_LOCATION: "android.permission.ACCESS_MEDIA_LOCATION";
  ACCEPT_HANDOVER: "android.permission.ACCEPT_HANDOVER";
  ACTIVITY_RECOGNITION: "android.permission.ACTIVITY_RECOGNITION";
  ANSWER_PHONE_CALLS: "android.permission.ANSWER_PHONE_CALLS";
  READ_PHONE_NUMBERS: "android.permission.READ_PHONE_NUMBERS";
  UWB_RANGING: "android.permission.UWB_RANGING";
  POST_NOTIFICATIONS: "android.permission.POST_NOTIFICATIONS";
  NEARBY_WIFI_DEVICES: "android.permission.NEARBY_WIFI_DEVICES";
}>;
export type PermissionStatus = "granted" | "denied" | "never_ask_again";
export type Permission = PermissionsType[keyof PermissionsType];
/**
 * `PermissionsAndroid` provides access to Android M's new permissions model.
 *
 * See https://reactnative.dev/docs/permissionsandroid
 */
declare class PermissionsAndroidImpl {
  PERMISSIONS: PermissionsType;
  RESULTS: Readonly<{
    DENIED: "denied";
    GRANTED: "granted";
    NEVER_ASK_AGAIN: "never_ask_again";
  }>;
  /**
   * DEPRECATED - use check
   *
   * Returns a promise resolving to a boolean value as to whether the specified
   * permissions has been granted
   *
   * @deprecated
   */
  checkPermission(permission: Permission): Promise<boolean>;
  /**
   * Returns a promise resolving to a boolean value as to whether the specified
   * permissions has been granted
   *
   * See https://reactnative.dev/docs/permissionsandroid#check
   */
  check(permission: Permission): Promise<boolean>;
  /**
   * DEPRECATED - use request
   *
   * Prompts the user to enable a permission and returns a promise resolving to a
   * boolean value indicating whether the user allowed or denied the request
   *
   * If the optional rationale argument is included (which is an object with a
   * `title` and `message`), this function checks with the OS whether it is
   * necessary to show a dialog explaining why the permission is needed
   * (https://developer.android.com/training/permissions/requesting#explain)
   * and then shows the system permission dialog
   *
   * @deprecated
   */
  requestPermission(permission: Permission, rationale?: Rationale): Promise<boolean>;
  /**
   * Prompts the user to enable a permission and returns a promise resolving to a
   * string value indicating whether the user allowed or denied the request
   *
   * See https://reactnative.dev/docs/permissionsandroid#request
   */
  request(permission: Permission, rationale?: Rationale): Promise<PermissionStatus>;
  /**
   * Prompts the user to enable multiple permissions in the same dialog and
   * returns an object with the permissions as keys and strings as values
   * indicating whether the user allowed or denied the request
   *
   * See https://reactnative.dev/docs/permissionsandroid#requestmultiple
   */
  requestMultiple(permissions: Array<Permission>): Promise<{ [permission in Permission]: PermissionStatus }>;
}
declare const PermissionsAndroidInstance: PermissionsAndroidImpl;
declare const $$PermissionsAndroid: typeof PermissionsAndroidInstance;
declare type $$PermissionsAndroid = typeof $$PermissionsAndroid;
export default $$PermissionsAndroid;

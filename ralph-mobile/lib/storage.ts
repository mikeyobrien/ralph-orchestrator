import * as SecureStore from "expo-secure-store";

const TOKEN_KEY = "token";
const USER_KEY = "user";
const SETTINGS_KEY = "settings";

// Token management
export async function saveToken(token: string): Promise<void> {
  await SecureStore.setItemAsync(TOKEN_KEY, token);
}

export async function getToken(): Promise<string | null> {
  return await SecureStore.getItemAsync(TOKEN_KEY);
}

export async function removeToken(): Promise<void> {
  await SecureStore.deleteItemAsync(TOKEN_KEY);
}

// User data management
export interface User {
  id: string;
  name: string;
  email: string;
}

export async function saveUser(user: User): Promise<void> {
  await SecureStore.setItemAsync(USER_KEY, JSON.stringify(user));
}

export async function getUser(): Promise<User | null> {
  const user = await SecureStore.getItemAsync(USER_KEY);
  return user ? JSON.parse(user) : null;
}

export async function removeUser(): Promise<void> {
  await SecureStore.deleteItemAsync(USER_KEY);
}

// Clear all auth data
export async function clearAuth(): Promise<void> {
  await Promise.all([
    SecureStore.deleteItemAsync(TOKEN_KEY),
    SecureStore.deleteItemAsync(USER_KEY),
  ]);
}

// Settings management
export interface AppSettings {
  notificationsEnabled: boolean;
  darkMode: boolean;
  autoRefreshInterval: number; // in seconds
  apiUrl?: string;
}

const defaultSettings: AppSettings = {
  notificationsEnabled: true,
  darkMode: true,
  autoRefreshInterval: 30,
};

export async function saveSettings(settings: Partial<AppSettings>): Promise<void> {
  const current = await getSettings();
  const merged = { ...current, ...settings };
  await SecureStore.setItemAsync(SETTINGS_KEY, JSON.stringify(merged));
}

export async function getSettings(): Promise<AppSettings> {
  const settings = await SecureStore.getItemAsync(SETTINGS_KEY);
  if (settings) {
    return { ...defaultSettings, ...JSON.parse(settings) };
  }
  return defaultSettings;
}

export async function resetSettings(): Promise<void> {
  await SecureStore.setItemAsync(SETTINGS_KEY, JSON.stringify(defaultSettings));
}

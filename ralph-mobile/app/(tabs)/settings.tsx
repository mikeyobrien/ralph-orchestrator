import { View, Text, ScrollView, Pressable, Switch } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { useState } from "react";
import { Ionicons } from "@expo/vector-icons";
import { useAuth } from "@/contexts/AuthContext";

type SettingItemProps = {
  icon: keyof typeof Ionicons.glyphMap;
  title: string;
  subtitle?: string;
  onPress?: () => void;
  rightElement?: React.ReactNode;
};

function SettingItem({ icon, title, subtitle, onPress, rightElement }: SettingItemProps) {
  return (
    <Pressable
      onPress={onPress}
      className="flex-row items-center bg-slate-800 rounded-xl p-4 mb-2 active:opacity-80"
    >
      <View className="w-10 h-10 bg-slate-700 rounded-full items-center justify-center mr-3">
        <Ionicons name={icon} size={20} color="#818cf8" />
      </View>
      <View className="flex-1">
        <Text className="text-white font-medium">{title}</Text>
        {subtitle && <Text className="text-slate-400 text-sm mt-0.5">{subtitle}</Text>}
      </View>
      {rightElement || (
        <Ionicons name="chevron-forward" size={20} color="#64748b" />
      )}
    </Pressable>
  );
}

export default function SettingsScreen() {
  const { user, logout } = useAuth();
  const [notifications, setNotifications] = useState(true);
  const [darkMode, setDarkMode] = useState(true);

  return (
    <SafeAreaView className="flex-1 bg-slate-900">
      {/* Header */}
      <View className="px-4 py-4 border-b border-slate-800">
        <Text className="text-2xl font-bold text-white">Settings</Text>
        <Text className="text-slate-400 mt-1">Configure your app preferences</Text>
      </View>

      <ScrollView className="flex-1 px-4">
        <View className="py-4">
          {/* Account Section */}
          <Text className="text-slate-400 text-sm font-medium mb-3">ACCOUNT</Text>
          <View className="bg-slate-800 rounded-xl p-4 mb-6">
            <View className="flex-row items-center">
              <View className="w-14 h-14 bg-indigo-600 rounded-full items-center justify-center mr-4">
                <Text className="text-white text-xl font-bold">
                  {user?.name?.charAt(0)?.toUpperCase() || "U"}
                </Text>
              </View>
              <View className="flex-1">
                <Text className="text-white font-semibold text-lg">
                  {user?.name || "User"}
                </Text>
                <Text className="text-slate-400">{user?.email || "No email"}</Text>
              </View>
            </View>
          </View>

          {/* Preferences */}
          <Text className="text-slate-400 text-sm font-medium mb-3">PREFERENCES</Text>
          <SettingItem
            icon="notifications-outline"
            title="Push Notifications"
            subtitle="Receive alerts for session updates"
            rightElement={
              <Switch
                value={notifications}
                onValueChange={setNotifications}
                trackColor={{ false: "#475569", true: "#6366f1" }}
                thumbColor="#fff"
              />
            }
          />
          <SettingItem
            icon="moon-outline"
            title="Dark Mode"
            subtitle="Use dark theme"
            rightElement={
              <Switch
                value={darkMode}
                onValueChange={setDarkMode}
                trackColor={{ false: "#475569", true: "#6366f1" }}
                thumbColor="#fff"
              />
            }
          />
          <View className="mb-4" />

          {/* Connection */}
          <Text className="text-slate-400 text-sm font-medium mb-3">CONNECTION</Text>
          <SettingItem
            icon="server-outline"
            title="API Server"
            subtitle="http://localhost:8000"
            onPress={() => {}}
          />
          <SettingItem
            icon="shield-checkmark-outline"
            title="Authentication"
            subtitle="Bearer token"
            onPress={() => {}}
          />
          <View className="mb-4" />

          {/* About */}
          <Text className="text-slate-400 text-sm font-medium mb-3">ABOUT</Text>
          <SettingItem
            icon="information-circle-outline"
            title="Version"
            subtitle="1.0.0"
            onPress={() => {}}
          />
          <SettingItem
            icon="help-circle-outline"
            title="Help & Support"
            onPress={() => {}}
          />
          <SettingItem
            icon="document-text-outline"
            title="Privacy Policy"
            onPress={() => {}}
          />
          <View className="mb-4" />

          {/* Logout */}
          <Pressable
            onPress={logout}
            className="bg-red-600/20 rounded-xl p-4 items-center active:bg-red-600/30"
          >
            <Text className="text-red-400 font-semibold">Sign Out</Text>
          </Pressable>
        </View>
      </ScrollView>
    </SafeAreaView>
  );
}

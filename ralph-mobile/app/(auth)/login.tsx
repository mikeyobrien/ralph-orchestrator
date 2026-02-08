import { useState } from "react";
import {
  View,
  Text,
  TextInput,
  TouchableOpacity,
  ActivityIndicator,
  KeyboardAvoidingView,
  Platform,
  Alert,
} from "react-native";
import { Link } from "expo-router";
import { useAuth } from "@/contexts/AuthContext";

export default function LoginScreen() {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const { login } = useAuth();

  async function handleLogin() {
    if (!email.trim() || !password.trim()) {
      Alert.alert("Error", "Please enter email and password");
      return;
    }

    setIsLoading(true);
    try {
      await login(email.trim(), password);
    } catch (error: unknown) {
      const message =
        error instanceof Error ? error.message : "Login failed. Please try again.";
      Alert.alert("Login Failed", message);
    } finally {
      setIsLoading(false);
    }
  }

  return (
    <KeyboardAvoidingView
      behavior={Platform.OS === "ios" ? "padding" : "height"}
      className="flex-1 bg-slate-900"
    >
      <View className="flex-1 justify-center px-6">
        {/* Header */}
        <View className="mb-10">
          <Text className="text-4xl font-bold text-white text-center mb-2">
            Ralph
          </Text>
          <Text className="text-slate-400 text-center text-lg">
            Orchestrator Control
          </Text>
        </View>

        {/* Login Form */}
        <View className="space-y-4">
          <View>
            <Text className="text-slate-300 mb-2 text-sm font-medium">
              Email
            </Text>
            <TextInput
              className="bg-slate-800 text-white px-4 py-3 rounded-lg border border-slate-700"
              placeholder="your@email.com"
              placeholderTextColor="#64748b"
              value={email}
              onChangeText={setEmail}
              autoCapitalize="none"
              autoCorrect={false}
              keyboardType="email-address"
              textContentType="emailAddress"
              autoComplete="email"
            />
          </View>

          <View className="mt-4">
            <Text className="text-slate-300 mb-2 text-sm font-medium">
              Password
            </Text>
            <TextInput
              className="bg-slate-800 text-white px-4 py-3 rounded-lg border border-slate-700"
              placeholder="Enter password"
              placeholderTextColor="#64748b"
              value={password}
              onChangeText={setPassword}
              secureTextEntry
              textContentType="password"
              autoComplete="password"
            />
          </View>

          <TouchableOpacity
            className={`mt-6 py-4 rounded-lg ${
              isLoading ? "bg-blue-600/50" : "bg-blue-600"
            }`}
            onPress={handleLogin}
            disabled={isLoading}
          >
            {isLoading ? (
              <ActivityIndicator color="white" />
            ) : (
              <Text className="text-white text-center font-semibold text-lg">
                Sign In
              </Text>
            )}
          </TouchableOpacity>
        </View>

        {/* Register Link */}
        <View className="mt-8 flex-row justify-center">
          <Text className="text-slate-400">Don't have an account? </Text>
          <Link href="/(auth)/register" asChild>
            <TouchableOpacity>
              <Text className="text-blue-400 font-semibold">Sign Up</Text>
            </TouchableOpacity>
          </Link>
        </View>

        {/* Skip Auth (Dev Mode) */}
        <View className="mt-12 items-center">
          <Link href="/" asChild>
            <TouchableOpacity className="py-2 px-4">
              <Text className="text-slate-500 text-sm">
                Skip (Development Mode)
              </Text>
            </TouchableOpacity>
          </Link>
        </View>
      </View>
    </KeyboardAvoidingView>
  );
}

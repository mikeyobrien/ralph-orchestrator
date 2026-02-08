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
  ScrollView,
} from "react-native";
import { Link } from "expo-router";
import { useAuth } from "@/contexts/AuthContext";

export default function RegisterScreen() {
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const { register } = useAuth();

  async function handleRegister() {
    // Validation
    if (!name.trim()) {
      Alert.alert("Error", "Please enter your name");
      return;
    }
    if (!email.trim()) {
      Alert.alert("Error", "Please enter your email");
      return;
    }
    if (!password.trim()) {
      Alert.alert("Error", "Please enter a password");
      return;
    }
    if (password.length < 8) {
      Alert.alert("Error", "Password must be at least 8 characters");
      return;
    }
    if (password !== confirmPassword) {
      Alert.alert("Error", "Passwords do not match");
      return;
    }

    setIsLoading(true);
    try {
      await register(name.trim(), email.trim(), password);
    } catch (error: unknown) {
      const message =
        error instanceof Error
          ? error.message
          : "Registration failed. Please try again.";
      Alert.alert("Registration Failed", message);
    } finally {
      setIsLoading(false);
    }
  }

  return (
    <KeyboardAvoidingView
      behavior={Platform.OS === "ios" ? "padding" : "height"}
      className="flex-1 bg-slate-900"
    >
      <ScrollView
        contentContainerStyle={{ flexGrow: 1 }}
        keyboardShouldPersistTaps="handled"
      >
        <View className="flex-1 justify-center px-6 py-10">
          {/* Header */}
          <View className="mb-8">
            <Text className="text-3xl font-bold text-white text-center mb-2">
              Create Account
            </Text>
            <Text className="text-slate-400 text-center">
              Join Ralph Orchestrator
            </Text>
          </View>

          {/* Register Form */}
          <View className="space-y-4">
            <View>
              <Text className="text-slate-300 mb-2 text-sm font-medium">
                Full Name
              </Text>
              <TextInput
                className="bg-slate-800 text-white px-4 py-3 rounded-lg border border-slate-700"
                placeholder="John Doe"
                placeholderTextColor="#64748b"
                value={name}
                onChangeText={setName}
                autoCapitalize="words"
                textContentType="name"
                autoComplete="name"
              />
            </View>

            <View className="mt-4">
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
                placeholder="Min 8 characters"
                placeholderTextColor="#64748b"
                value={password}
                onChangeText={setPassword}
                secureTextEntry
                textContentType="newPassword"
                autoComplete="password-new"
              />
            </View>

            <View className="mt-4">
              <Text className="text-slate-300 mb-2 text-sm font-medium">
                Confirm Password
              </Text>
              <TextInput
                className="bg-slate-800 text-white px-4 py-3 rounded-lg border border-slate-700"
                placeholder="Re-enter password"
                placeholderTextColor="#64748b"
                value={confirmPassword}
                onChangeText={setConfirmPassword}
                secureTextEntry
                textContentType="newPassword"
                autoComplete="password-new"
              />
            </View>

            <TouchableOpacity
              className={`mt-6 py-4 rounded-lg ${
                isLoading ? "bg-green-600/50" : "bg-green-600"
              }`}
              onPress={handleRegister}
              disabled={isLoading}
            >
              {isLoading ? (
                <ActivityIndicator color="white" />
              ) : (
                <Text className="text-white text-center font-semibold text-lg">
                  Create Account
                </Text>
              )}
            </TouchableOpacity>
          </View>

          {/* Login Link */}
          <View className="mt-8 flex-row justify-center">
            <Text className="text-slate-400">Already have an account? </Text>
            <Link href="/(auth)/login" asChild>
              <TouchableOpacity>
                <Text className="text-blue-400 font-semibold">Sign In</Text>
              </TouchableOpacity>
            </Link>
          </View>
        </View>
      </ScrollView>
    </KeyboardAvoidingView>
  );
}

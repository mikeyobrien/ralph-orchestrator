import {
  View,
  Text,
  ScrollView,
  Pressable,
  RefreshControl,
  Alert,
  ActivityIndicator,
} from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { useLocalSearchParams, useRouter, Stack } from "expo-router";
import { useState, useCallback, useEffect } from "react";
import { orchestratorApi } from "../../lib/api";

// Types
interface Iteration {
  id: string;
  number: number;
  status: "pending" | "running" | "completed" | "failed";
  startedAt?: string;
  completedAt?: string;
  duration?: number;
  tokensUsed?: number;
  cost?: number;
  summary?: string;
}

interface SessionDetail {
  id: string;
  name: string;
  status: "running" | "completed" | "paused" | "failed" | "pending";
  progress: number;
  currentIteration: number;
  totalIterations: number;
  startedAt: string;
  completedAt?: string;
  promptFile: string;
  totalTokens: number;
  totalCost: number;
  iterations: Iteration[];
}

// Mock data for development (will be replaced with real API calls)
const mockSessionDetail: SessionDetail = {
  id: "session-001",
  name: "Feature Implementation",
  status: "running",
  progress: 45,
  currentIteration: 5,
  totalIterations: 12,
  startedAt: new Date(Date.now() - 1000 * 60 * 30).toISOString(),
  promptFile: "prompts/mobile/PROMPT.md",
  totalTokens: 125000,
  totalCost: 2.45,
  iterations: [
    {
      id: "iter-001",
      number: 1,
      status: "completed",
      startedAt: new Date(Date.now() - 1000 * 60 * 28).toISOString(),
      completedAt: new Date(Date.now() - 1000 * 60 * 26).toISOString(),
      duration: 120,
      tokensUsed: 15000,
      cost: 0.30,
      summary: "Project structure setup",
    },
    {
      id: "iter-002",
      number: 2,
      status: "completed",
      startedAt: new Date(Date.now() - 1000 * 60 * 26).toISOString(),
      completedAt: new Date(Date.now() - 1000 * 60 * 23).toISOString(),
      duration: 180,
      tokensUsed: 22000,
      cost: 0.44,
      summary: "API client implementation",
    },
    {
      id: "iter-003",
      number: 3,
      status: "completed",
      startedAt: new Date(Date.now() - 1000 * 60 * 23).toISOString(),
      completedAt: new Date(Date.now() - 1000 * 60 * 19).toISOString(),
      duration: 240,
      tokensUsed: 28000,
      cost: 0.56,
      summary: "Authentication context",
    },
    {
      id: "iter-004",
      number: 4,
      status: "completed",
      startedAt: new Date(Date.now() - 1000 * 60 * 19).toISOString(),
      completedAt: new Date(Date.now() - 1000 * 60 * 14).toISOString(),
      duration: 300,
      tokensUsed: 32000,
      cost: 0.64,
      summary: "Auth screens",
    },
    {
      id: "iter-005",
      number: 5,
      status: "running",
      startedAt: new Date(Date.now() - 1000 * 60 * 14).toISOString(),
      tokensUsed: 28000,
      cost: 0.51,
      summary: "Tab navigation setup",
    },
  ],
};

type SessionStatus = "running" | "completed" | "paused" | "failed" | "pending";
type IterationStatus = "pending" | "running" | "completed" | "failed";

function getStatusColor(status: SessionStatus | IterationStatus): string {
  switch (status) {
    case "running":
      return "bg-emerald-500";
    case "completed":
      return "bg-indigo-500";
    case "paused":
      return "bg-amber-500";
    case "failed":
      return "bg-red-500";
    case "pending":
      return "bg-slate-500";
    default:
      return "bg-slate-500";
  }
}

function getStatusText(status: SessionStatus | IterationStatus): string {
  return status.charAt(0).toUpperCase() + status.slice(1);
}

function formatDuration(seconds: number): string {
  if (seconds < 60) {
    return `${seconds}s`;
  }
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return secs > 0 ? `${mins}m ${secs}s` : `${mins}m`;
}

function formatTokens(tokens: number): string {
  if (tokens >= 1000000) {
    return `${(tokens / 1000000).toFixed(1)}M`;
  }
  if (tokens >= 1000) {
    return `${(tokens / 1000).toFixed(1)}K`;
  }
  return tokens.toString();
}

function formatCost(cost: number): string {
  return `$${cost.toFixed(2)}`;
}

function formatDateTime(dateString: string): string {
  const date = new Date(dateString);
  return date.toLocaleTimeString("en-US", {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export default function SessionDetailScreen() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const router = useRouter();
  const [session, setSession] = useState<SessionDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [actionLoading, setActionLoading] = useState<string | null>(null);

  // Fetch session data
  const fetchSession = useCallback(async () => {
    try {
      // In production, use real API:
      // const response = await orchestratorApi.getSession(id);
      // const iterationsResponse = await orchestratorApi.getIterations(id);
      // setSession({ ...response.data, iterations: iterationsResponse.data });

      // For now, use mock data
      await new Promise((resolve) => setTimeout(resolve, 500));
      setSession(mockSessionDetail);
    } catch (error) {
      console.error("Error fetching session:", error);
      Alert.alert("Error", "Failed to load session details");
    } finally {
      setLoading(false);
    }
  }, [id]);

  useEffect(() => {
    fetchSession();
  }, [fetchSession]);

  const onRefresh = useCallback(async () => {
    setRefreshing(true);
    await fetchSession();
    setRefreshing(false);
  }, [fetchSession]);

  // Session control actions
  const handlePause = useCallback(async () => {
    if (!session || session.status !== "running") return;

    Alert.alert(
      "Pause Session",
      "Are you sure you want to pause this session?",
      [
        { text: "Cancel", style: "cancel" },
        {
          text: "Pause",
          style: "destructive",
          onPress: async () => {
            setActionLoading("pause");
            try {
              // In production: await orchestratorApi.pauseSession(id);
              await new Promise((resolve) => setTimeout(resolve, 500));
              setSession((prev) => prev ? { ...prev, status: "paused" } : null);
            } catch (error) {
              Alert.alert("Error", "Failed to pause session");
            } finally {
              setActionLoading(null);
            }
          },
        },
      ]
    );
  }, [session, id]);

  const handleResume = useCallback(async () => {
    if (!session || session.status !== "paused") return;

    setActionLoading("resume");
    try {
      // In production: await orchestratorApi.resumeSession(id);
      await new Promise((resolve) => setTimeout(resolve, 500));
      setSession((prev) => prev ? { ...prev, status: "running" } : null);
    } catch (error) {
      Alert.alert("Error", "Failed to resume session");
    } finally {
      setActionLoading(null);
    }
  }, [session, id]);

  const handleStop = useCallback(async () => {
    if (!session || session.status === "completed" || session.status === "failed") return;

    Alert.alert(
      "Stop Session",
      "Are you sure you want to stop this session? This action cannot be undone.",
      [
        { text: "Cancel", style: "cancel" },
        {
          text: "Stop",
          style: "destructive",
          onPress: async () => {
            setActionLoading("stop");
            try {
              // In production: await orchestratorApi.stopSession(id);
              await new Promise((resolve) => setTimeout(resolve, 500));
              setSession((prev) => prev ? { ...prev, status: "failed" } : null);
            } catch (error) {
              Alert.alert("Error", "Failed to stop session");
            } finally {
              setActionLoading(null);
            }
          },
        },
      ]
    );
  }, [session, id]);

  if (loading) {
    return (
      <SafeAreaView className="flex-1 bg-slate-900 items-center justify-center">
        <ActivityIndicator size="large" color="#818cf8" />
        <Text className="text-slate-400 mt-4">Loading session...</Text>
      </SafeAreaView>
    );
  }

  if (!session) {
    return (
      <SafeAreaView className="flex-1 bg-slate-900 items-center justify-center">
        <Text className="text-slate-400">Session not found</Text>
        <Pressable
          onPress={() => router.back()}
          className="mt-4 bg-indigo-600 px-6 py-3 rounded-lg"
        >
          <Text className="text-white font-semibold">Go Back</Text>
        </Pressable>
      </SafeAreaView>
    );
  }

  const canPause = session.status === "running";
  const canResume = session.status === "paused";
  const canStop = session.status === "running" || session.status === "paused";

  return (
    <>
      <Stack.Screen
        options={{
          title: session.name,
          headerStyle: { backgroundColor: "#0f172a" },
          headerTintColor: "#fff",
        }}
      />
      <SafeAreaView className="flex-1 bg-slate-900" edges={["bottom"]}>
        <ScrollView
          className="flex-1"
          refreshControl={
            <RefreshControl
              refreshing={refreshing}
              onRefresh={onRefresh}
              tintColor="#818cf8"
            />
          }
        >
          {/* Status & Progress Section */}
          <View className="px-4 py-4 border-b border-slate-800">
            <View className="flex-row items-center justify-between mb-4">
              <View
                className={`px-3 py-1.5 rounded-full ${getStatusColor(session.status)}`}
              >
                <Text className="text-white text-sm font-semibold">
                  {getStatusText(session.status)}
                </Text>
              </View>
              <Text className="text-slate-400 text-sm">
                Started {formatDateTime(session.startedAt)}
              </Text>
            </View>

            {/* Progress Bar */}
            <View className="mb-2">
              <View className="flex-row justify-between mb-1">
                <Text className="text-white font-medium">Progress</Text>
                <Text className="text-slate-400">{session.progress}%</Text>
              </View>
              <View className="h-3 bg-slate-700 rounded-full">
                <View
                  className={`h-3 rounded-full ${getStatusColor(session.status)}`}
                  style={{ width: `${session.progress}%` }}
                />
              </View>
            </View>

            <Text className="text-slate-400 text-sm mt-2">
              Iteration {session.currentIteration} of {session.totalIterations}
            </Text>
          </View>

          {/* Control Actions */}
          <View className="px-4 py-4 border-b border-slate-800">
            <Text className="text-slate-400 text-sm font-medium mb-3">
              SESSION CONTROLS
            </Text>
            <View className="flex-row gap-3">
              {canPause && (
                <Pressable
                  onPress={handlePause}
                  disabled={actionLoading !== null}
                  className="flex-1 bg-amber-600 rounded-xl p-3 items-center active:bg-amber-700 disabled:opacity-50"
                >
                  {actionLoading === "pause" ? (
                    <ActivityIndicator color="#fff" size="small" />
                  ) : (
                    <Text className="text-white font-semibold">Pause</Text>
                  )}
                </Pressable>
              )}
              {canResume && (
                <Pressable
                  onPress={handleResume}
                  disabled={actionLoading !== null}
                  className="flex-1 bg-emerald-600 rounded-xl p-3 items-center active:bg-emerald-700 disabled:opacity-50"
                >
                  {actionLoading === "resume" ? (
                    <ActivityIndicator color="#fff" size="small" />
                  ) : (
                    <Text className="text-white font-semibold">Resume</Text>
                  )}
                </Pressable>
              )}
              {canStop && (
                <Pressable
                  onPress={handleStop}
                  disabled={actionLoading !== null}
                  className="flex-1 bg-red-600 rounded-xl p-3 items-center active:bg-red-700 disabled:opacity-50"
                >
                  {actionLoading === "stop" ? (
                    <ActivityIndicator color="#fff" size="small" />
                  ) : (
                    <Text className="text-white font-semibold">Stop</Text>
                  )}
                </Pressable>
              )}
              {!canPause && !canResume && !canStop && (
                <View className="flex-1 bg-slate-700 rounded-xl p-3 items-center">
                  <Text className="text-slate-400 font-semibold">
                    Session {session.status}
                  </Text>
                </View>
              )}
            </View>
          </View>

          {/* Metrics Summary */}
          <View className="px-4 py-4 border-b border-slate-800">
            <Text className="text-slate-400 text-sm font-medium mb-3">
              METRICS
            </Text>
            <View className="flex-row gap-3">
              <View className="flex-1 bg-slate-800 rounded-xl p-4">
                <Text className="text-slate-400 text-xs mb-1">Tokens Used</Text>
                <Text className="text-white text-xl font-bold">
                  {formatTokens(session.totalTokens)}
                </Text>
              </View>
              <View className="flex-1 bg-slate-800 rounded-xl p-4">
                <Text className="text-slate-400 text-xs mb-1">Total Cost</Text>
                <Text className="text-white text-xl font-bold">
                  {formatCost(session.totalCost)}
                </Text>
              </View>
            </View>
          </View>

          {/* Prompt File */}
          <View className="px-4 py-4 border-b border-slate-800">
            <Text className="text-slate-400 text-sm font-medium mb-2">
              PROMPT FILE
            </Text>
            <View className="bg-slate-800 rounded-xl p-3">
              <Text className="text-white font-mono text-sm">
                {session.promptFile}
              </Text>
            </View>
          </View>

          {/* Iterations List */}
          <View className="px-4 py-4">
            <Text className="text-slate-400 text-sm font-medium mb-3">
              ITERATIONS ({session.iterations.length})
            </Text>

            {session.iterations.map((iteration) => (
              <View
                key={iteration.id}
                className="bg-slate-800 rounded-xl p-4 mb-3"
              >
                {/* Iteration Header */}
                <View className="flex-row items-center justify-between mb-2">
                  <View className="flex-row items-center">
                    <View
                      className={`w-8 h-8 rounded-full items-center justify-center ${getStatusColor(iteration.status)}`}
                    >
                      <Text className="text-white font-bold text-sm">
                        {iteration.number}
                      </Text>
                    </View>
                    <Text className="text-white font-semibold ml-3">
                      Iteration {iteration.number}
                    </Text>
                  </View>
                  <View
                    className={`px-2 py-1 rounded-full ${getStatusColor(iteration.status)}`}
                  >
                    <Text className="text-white text-xs font-medium">
                      {getStatusText(iteration.status)}
                    </Text>
                  </View>
                </View>

                {/* Iteration Summary */}
                {iteration.summary && (
                  <Text className="text-slate-300 text-sm mb-2">
                    {iteration.summary}
                  </Text>
                )}

                {/* Iteration Metrics */}
                <View className="flex-row justify-between mt-2">
                  {iteration.duration && (
                    <Text className="text-slate-400 text-xs">
                      ⏱ {formatDuration(iteration.duration)}
                    </Text>
                  )}
                  {iteration.tokensUsed && (
                    <Text className="text-slate-400 text-xs">
                      📊 {formatTokens(iteration.tokensUsed)} tokens
                    </Text>
                  )}
                  {iteration.cost && (
                    <Text className="text-slate-400 text-xs">
                      💰 {formatCost(iteration.cost)}
                    </Text>
                  )}
                </View>

                {/* Running indicator */}
                {iteration.status === "running" && (
                  <View className="flex-row items-center mt-3 pt-3 border-t border-slate-700">
                    <ActivityIndicator size="small" color="#10b981" />
                    <Text className="text-emerald-400 text-sm ml-2">
                      In progress...
                    </Text>
                  </View>
                )}
              </View>
            ))}
          </View>
        </ScrollView>
      </SafeAreaView>
    </>
  );
}

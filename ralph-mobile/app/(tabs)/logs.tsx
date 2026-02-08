import { View, Text, ScrollView, RefreshControl } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { useState, useCallback } from "react";

// Mock log data for development
const mockLogs = [
  {
    id: "log-001",
    timestamp: new Date(Date.now() - 1000 * 60 * 2).toISOString(),
    level: "info",
    message: "Iteration 5 started - executing subtask",
    sessionId: "session-001",
  },
  {
    id: "log-002",
    timestamp: new Date(Date.now() - 1000 * 60 * 5).toISOString(),
    level: "success",
    message: "Iteration 4 completed successfully",
    sessionId: "session-001",
  },
  {
    id: "log-003",
    timestamp: new Date(Date.now() - 1000 * 60 * 8).toISOString(),
    level: "warning",
    message: "High token usage detected (85% of limit)",
    sessionId: "session-001",
  },
  {
    id: "log-004",
    timestamp: new Date(Date.now() - 1000 * 60 * 12).toISOString(),
    level: "info",
    message: "API rate limit approaching, throttling requests",
    sessionId: "session-001",
  },
  {
    id: "log-005",
    timestamp: new Date(Date.now() - 1000 * 60 * 15).toISOString(),
    level: "error",
    message: "Failed to connect to remote service, retrying...",
    sessionId: "session-002",
  },
];

type LogLevel = "info" | "success" | "warning" | "error";

function getLogLevelColor(level: LogLevel): string {
  switch (level) {
    case "info":
      return "text-blue-400";
    case "success":
      return "text-emerald-400";
    case "warning":
      return "text-amber-400";
    case "error":
      return "text-red-400";
    default:
      return "text-slate-400";
  }
}

function getLogLevelBg(level: LogLevel): string {
  switch (level) {
    case "info":
      return "bg-blue-500/20";
    case "success":
      return "bg-emerald-500/20";
    case "warning":
      return "bg-amber-500/20";
    case "error":
      return "bg-red-500/20";
    default:
      return "bg-slate-500/20";
  }
}

function formatLogTime(dateString: string): string {
  const date = new Date(dateString);
  return date.toLocaleTimeString("en-US", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export default function LogsScreen() {
  const [refreshing, setRefreshing] = useState(false);
  const [logs] = useState(mockLogs);

  const onRefresh = useCallback(() => {
    setRefreshing(true);
    setTimeout(() => {
      setRefreshing(false);
    }, 1000);
  }, []);

  return (
    <SafeAreaView className="flex-1 bg-slate-900">
      {/* Header */}
      <View className="px-4 py-4 border-b border-slate-800">
        <Text className="text-2xl font-bold text-white">Logs</Text>
        <Text className="text-slate-400 mt-1">Real-time execution logs</Text>
      </View>

      {/* Log List */}
      <ScrollView
        className="flex-1 px-4"
        refreshControl={
          <RefreshControl
            refreshing={refreshing}
            onRefresh={onRefresh}
            tintColor="#818cf8"
          />
        }
      >
        <View className="py-4">
          {logs.map((log) => (
            <View
              key={log.id}
              className={`rounded-lg p-3 mb-2 ${getLogLevelBg(log.level as LogLevel)}`}
            >
              <View className="flex-row items-center justify-between mb-1">
                <Text className={`font-semibold uppercase text-xs ${getLogLevelColor(log.level as LogLevel)}`}>
                  {log.level}
                </Text>
                <Text className="text-slate-500 text-xs">
                  {formatLogTime(log.timestamp)}
                </Text>
              </View>
              <Text className="text-slate-200 text-sm">{log.message}</Text>
              <Text className="text-slate-500 text-xs mt-1">{log.sessionId}</Text>
            </View>
          ))}
        </View>
      </ScrollView>
    </SafeAreaView>
  );
}

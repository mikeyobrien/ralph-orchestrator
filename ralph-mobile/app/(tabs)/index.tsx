import { View, Text, ScrollView, RefreshControl } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { useRouter } from "expo-router";
import { useState, useCallback, useMemo, memo } from "react";
import { AnimatedCard, SessionCardSkeleton, FadeIn } from "../../components";

// Mock data for development
const mockSessions = [
  {
    id: "session-001",
    name: "Feature Implementation",
    status: "running",
    progress: 45,
    currentIteration: 5,
    totalIterations: 12,
    startedAt: new Date(Date.now() - 1000 * 60 * 30).toISOString(),
  },
  {
    id: "session-002",
    name: "Bug Fix Sprint",
    status: "completed",
    progress: 100,
    currentIteration: 8,
    totalIterations: 8,
    startedAt: new Date(Date.now() - 1000 * 60 * 120).toISOString(),
  },
  {
    id: "session-003",
    name: "Refactoring Task",
    status: "paused",
    progress: 30,
    currentIteration: 3,
    totalIterations: 10,
    startedAt: new Date(Date.now() - 1000 * 60 * 60).toISOString(),
  },
];

type SessionStatus = "running" | "completed" | "paused" | "failed";

function getStatusColor(status: SessionStatus): string {
  switch (status) {
    case "running":
      return "bg-emerald-500";
    case "completed":
      return "bg-indigo-500";
    case "paused":
      return "bg-amber-500";
    case "failed":
      return "bg-red-500";
    default:
      return "bg-slate-500";
  }
}

function getStatusText(status: SessionStatus): string {
  return status.charAt(0).toUpperCase() + status.slice(1);
}

function formatTimeAgo(dateString: string): string {
  const date = new Date(dateString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / (1000 * 60));

  if (diffMins < 60) {
    return `${diffMins}m ago`;
  }
  const diffHours = Math.floor(diffMins / 60);
  if (diffHours < 24) {
    return `${diffHours}h ago`;
  }
  const diffDays = Math.floor(diffHours / 24);
  return `${diffDays}d ago`;
}

// Session type for memoized component
type Session = {
  id: string;
  name: string;
  status: string;
  progress: number;
  currentIteration: number;
  totalIterations: number;
  startedAt: string;
};

// Memoized session card component for performance
const SessionCard = memo(function SessionCard({
  session,
  index,
  onPress,
}: {
  session: Session;
  index: number;
  onPress: () => void;
}) {
  return (
    <AnimatedCard
      index={index}
      staggerDelay={80}
      pressable
      onPress={onPress}
      style={{
        backgroundColor: "#1e293b",
        borderRadius: 12,
        padding: 16,
        marginBottom: 12,
      }}
    >
      {/* Session Header */}
      <View className="flex-row items-center justify-between mb-3">
        <Text className="text-white font-semibold text-lg flex-1">
          {session.name}
        </Text>
        <View className={`px-2 py-1 rounded-full ${getStatusColor(session.status as SessionStatus)}`}>
          <Text className="text-white text-xs font-medium">
            {getStatusText(session.status as SessionStatus)}
          </Text>
        </View>
      </View>

      {/* Progress Bar */}
      <View className="h-2 bg-slate-700 rounded-full mb-2">
        <View
          className={`h-2 rounded-full ${getStatusColor(session.status as SessionStatus)}`}
          style={{ width: `${session.progress}%` }}
        />
      </View>

      {/* Session Details */}
      <View className="flex-row justify-between">
        <Text className="text-slate-400 text-sm">
          Iteration {session.currentIteration}/{session.totalIterations}
        </Text>
        <Text className="text-slate-400 text-sm">
          {formatTimeAgo(session.startedAt)}
        </Text>
      </View>
    </AnimatedCard>
  );
});

// Loading skeleton for dashboard
function DashboardSkeleton() {
  return (
    <View className="py-4">
      <View className="mb-3 h-4 w-32 bg-slate-700 rounded" />
      <SessionCardSkeleton />
      <SessionCardSkeleton />
      <SessionCardSkeleton />
    </View>
  );
}

export default function Dashboard() {
  const router = useRouter();
  const [refreshing, setRefreshing] = useState(false);
  const [loading, setLoading] = useState(true);
  const [sessions, setSessions] = useState<Session[]>([]);

  // Simulate initial data fetch
  const fetchSessions = useCallback(async () => {
    // In production: const response = await orchestratorApi.getSessions();
    await new Promise((resolve) => setTimeout(resolve, 800));
    setSessions(mockSessions);
    setLoading(false);
  }, []);

  // Initial load
  useMemo(() => {
    fetchSessions();
  }, [fetchSessions]);

  const onRefresh = useCallback(async () => {
    setRefreshing(true);
    await fetchSessions();
    setRefreshing(false);
  }, [fetchSessions]);

  // Memoize navigation callbacks to prevent re-renders
  const handleSessionPress = useCallback(
    (sessionId: string) => {
      router.push(`/session/${sessionId}`);
    },
    [router]
  );

  return (
    <SafeAreaView className="flex-1 bg-slate-900">
      {/* Header */}
      <FadeIn>
        <View className="px-4 py-4 border-b border-slate-800">
          <Text className="text-2xl font-bold text-white">Ralph Orchestrator</Text>
          <Text className="text-slate-400 mt-1">Session Dashboard</Text>
        </View>
      </FadeIn>

      {/* Session List */}
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
        {loading ? (
          <DashboardSkeleton />
        ) : (
          <View className="py-4">
            <FadeIn delay={100}>
              <Text className="text-slate-400 text-sm font-medium mb-3">
                ACTIVE SESSIONS ({sessions.length})
              </Text>
            </FadeIn>

            {sessions.map((session, index) => (
              <SessionCard
                key={session.id}
                session={session}
                index={index}
                onPress={() => handleSessionPress(session.id)}
              />
            ))}
          </View>
        )}

        {/* Quick Actions */}
        {!loading && (
          <AnimatedCard
            delay={sessions.length * 80 + 100}
            style={{ paddingVertical: 16, borderTopWidth: 1, borderTopColor: "#1e293b" }}
          >
            <Text className="text-slate-400 text-sm font-medium mb-3">
              QUICK ACTIONS
            </Text>

            <View className="flex-row gap-3">
              <AnimatedCard
                delay={sessions.length * 80 + 150}
                pressable
                style={{
                  flex: 1,
                  backgroundColor: "#4f46e5",
                  borderRadius: 12,
                  padding: 16,
                  alignItems: "center",
                }}
              >
                <Text className="text-white font-semibold">New Session</Text>
              </AnimatedCard>
              <AnimatedCard
                delay={sessions.length * 80 + 200}
                pressable
                onPress={() => router.push("/(tabs)/logs")}
                style={{
                  flex: 1,
                  backgroundColor: "#1e293b",
                  borderRadius: 12,
                  padding: 16,
                  alignItems: "center",
                }}
              >
                <Text className="text-white font-semibold">View Logs</Text>
              </AnimatedCard>
            </View>
          </AnimatedCard>
        )}
      </ScrollView>
    </SafeAreaView>
  );
}

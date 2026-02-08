import { View, Text, ScrollView, RefreshControl } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { useState, useCallback } from "react";

// Mock metrics data for development
const mockMetrics = {
  totalSessions: 24,
  activeSessions: 3,
  completedToday: 8,
  failedToday: 1,
  totalTokens: 1250000,
  totalCost: 45.32,
  avgIterationTime: 42,
  successRate: 95.5,
};

type MetricCardProps = {
  title: string;
  value: string | number;
  subtitle?: string;
  color?: string;
};

function MetricCard({ title, value, subtitle, color = "text-white" }: MetricCardProps) {
  return (
    <View className="bg-slate-800 rounded-xl p-4 flex-1">
      <Text className="text-slate-400 text-sm font-medium mb-1">{title}</Text>
      <Text className={`text-2xl font-bold ${color}`}>{value}</Text>
      {subtitle && <Text className="text-slate-500 text-xs mt-1">{subtitle}</Text>}
    </View>
  );
}

export default function MetricsScreen() {
  const [refreshing, setRefreshing] = useState(false);
  const [metrics] = useState(mockMetrics);

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
        <Text className="text-2xl font-bold text-white">Metrics</Text>
        <Text className="text-slate-400 mt-1">Usage and performance statistics</Text>
      </View>

      {/* Metrics Grid */}
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
          {/* Sessions Overview */}
          <Text className="text-slate-400 text-sm font-medium mb-3">SESSIONS</Text>
          <View className="flex-row gap-3 mb-4">
            <MetricCard
              title="Total"
              value={metrics.totalSessions}
              color="text-white"
            />
            <MetricCard
              title="Active"
              value={metrics.activeSessions}
              color="text-emerald-400"
            />
          </View>
          <View className="flex-row gap-3 mb-6">
            <MetricCard
              title="Completed Today"
              value={metrics.completedToday}
              color="text-indigo-400"
            />
            <MetricCard
              title="Failed Today"
              value={metrics.failedToday}
              color="text-red-400"
            />
          </View>

          {/* Token Usage */}
          <Text className="text-slate-400 text-sm font-medium mb-3">TOKEN USAGE</Text>
          <View className="bg-slate-800 rounded-xl p-4 mb-6">
            <Text className="text-slate-400 text-sm mb-2">Total Tokens Used</Text>
            <Text className="text-3xl font-bold text-white">
              {(metrics.totalTokens / 1000000).toFixed(2)}M
            </Text>
            <View className="h-2 bg-slate-700 rounded-full mt-3">
              <View
                className="h-2 bg-indigo-500 rounded-full"
                style={{ width: "62%" }}
              />
            </View>
            <Text className="text-slate-500 text-xs mt-2">62% of monthly limit</Text>
          </View>

          {/* Cost */}
          <Text className="text-slate-400 text-sm font-medium mb-3">COSTS</Text>
          <View className="bg-slate-800 rounded-xl p-4 mb-6">
            <Text className="text-slate-400 text-sm mb-2">Total Cost (This Month)</Text>
            <Text className="text-3xl font-bold text-emerald-400">
              ${metrics.totalCost.toFixed(2)}
            </Text>
            <Text className="text-slate-500 text-xs mt-2">
              Avg ${(metrics.totalCost / metrics.completedToday).toFixed(2)} per session
            </Text>
          </View>

          {/* Performance */}
          <Text className="text-slate-400 text-sm font-medium mb-3">PERFORMANCE</Text>
          <View className="flex-row gap-3 mb-6">
            <MetricCard
              title="Avg Iteration"
              value={`${metrics.avgIterationTime}s`}
              subtitle="Per iteration"
              color="text-amber-400"
            />
            <MetricCard
              title="Success Rate"
              value={`${metrics.successRate}%`}
              subtitle="Last 30 days"
              color="text-emerald-400"
            />
          </View>
        </View>
      </ScrollView>
    </SafeAreaView>
  );
}

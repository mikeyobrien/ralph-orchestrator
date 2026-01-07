import { useEffect, useRef } from "react";
import { View, Animated, StyleSheet, ViewStyle } from "react-native";

interface SkeletonProps {
  /** Width of skeleton (number for pixels, string for percentage) */
  width?: number | string;
  /** Height of skeleton */
  height?: number;
  /** Border radius */
  borderRadius?: number;
  /** Additional styles */
  style?: ViewStyle;
  /** Whether to show animation (default: true) */
  animated?: boolean;
}

/**
 * Skeleton loader component with shimmer animation.
 * Used for loading states to indicate content is being fetched.
 *
 * @example
 * // Basic usage
 * <Skeleton width={200} height={20} />
 *
 * // Full width card skeleton
 * <Skeleton width="100%" height={80} borderRadius={12} />
 *
 * // Circle avatar skeleton
 * <Skeleton width={48} height={48} borderRadius={24} />
 */
export function Skeleton({
  width = "100%",
  height = 20,
  borderRadius = 4,
  style,
  animated = true,
}: SkeletonProps) {
  const shimmerAnimation = useRef(new Animated.Value(0)).current;

  useEffect(() => {
    if (!animated) return;

    const animation = Animated.loop(
      Animated.sequence([
        Animated.timing(shimmerAnimation, {
          toValue: 1,
          duration: 1000,
          useNativeDriver: true,
        }),
        Animated.timing(shimmerAnimation, {
          toValue: 0,
          duration: 1000,
          useNativeDriver: true,
        }),
      ])
    );
    animation.start();

    return () => animation.stop();
  }, [shimmerAnimation, animated]);

  const opacity = animated
    ? shimmerAnimation.interpolate({
        inputRange: [0, 1],
        outputRange: [0.3, 0.7],
      })
    : 0.5;

  return (
    <Animated.View
      style={[
        styles.skeleton,
        {
          width,
          height,
          borderRadius,
          opacity,
        },
        style,
      ]}
    />
  );
}

/**
 * Pre-built skeleton for session cards matching Dashboard layout.
 */
export function SessionCardSkeleton() {
  return (
    <View style={styles.sessionCard}>
      {/* Header row: title + status badge */}
      <View style={styles.headerRow}>
        <Skeleton width="60%" height={24} borderRadius={4} />
        <Skeleton width={70} height={26} borderRadius={13} />
      </View>

      {/* Progress bar */}
      <Skeleton
        width="100%"
        height={8}
        borderRadius={4}
        style={{ marginTop: 12, marginBottom: 8 }}
      />

      {/* Footer row: iteration + time */}
      <View style={styles.footerRow}>
        <Skeleton width={100} height={16} borderRadius={4} />
        <Skeleton width={60} height={16} borderRadius={4} />
      </View>
    </View>
  );
}

/**
 * Pre-built skeleton for iteration items.
 */
export function IterationSkeleton() {
  return (
    <View style={styles.iterationCard}>
      {/* Header: number circle + title + status */}
      <View style={styles.headerRow}>
        <View style={{ flexDirection: "row", alignItems: "center" }}>
          <Skeleton width={32} height={32} borderRadius={16} />
          <Skeleton
            width={100}
            height={20}
            borderRadius={4}
            style={{ marginLeft: 12 }}
          />
        </View>
        <Skeleton width={60} height={22} borderRadius={11} />
      </View>

      {/* Summary text */}
      <Skeleton
        width="80%"
        height={16}
        borderRadius={4}
        style={{ marginTop: 8 }}
      />

      {/* Metrics row */}
      <View style={[styles.footerRow, { marginTop: 12 }]}>
        <Skeleton width={50} height={14} borderRadius={4} />
        <Skeleton width={80} height={14} borderRadius={4} />
        <Skeleton width={50} height={14} borderRadius={4} />
      </View>
    </View>
  );
}

/**
 * Pre-built skeleton for metrics cards.
 */
export function MetricCardSkeleton() {
  return (
    <View style={styles.metricCard}>
      <Skeleton width={80} height={12} borderRadius={4} />
      <Skeleton
        width={60}
        height={28}
        borderRadius={4}
        style={{ marginTop: 8 }}
      />
    </View>
  );
}

const styles = StyleSheet.create({
  skeleton: {
    backgroundColor: "#334155", // slate-700
  },
  sessionCard: {
    backgroundColor: "#1e293b", // slate-800
    borderRadius: 12,
    padding: 16,
    marginBottom: 12,
  },
  iterationCard: {
    backgroundColor: "#1e293b", // slate-800
    borderRadius: 12,
    padding: 16,
    marginBottom: 12,
  },
  metricCard: {
    backgroundColor: "#1e293b", // slate-800
    borderRadius: 12,
    padding: 16,
    flex: 1,
  },
  headerRow: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
  },
  footerRow: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
  },
});

export default Skeleton;

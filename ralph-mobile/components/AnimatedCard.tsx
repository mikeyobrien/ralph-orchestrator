import { useEffect, useRef } from "react";
import { Animated, ViewStyle, Pressable, PressableProps } from "react-native";

interface AnimatedCardProps extends Omit<PressableProps, "style"> {
  /** Child components */
  children: React.ReactNode;
  /** Delay before animation starts (ms) */
  delay?: number;
  /** Animation duration (ms) */
  duration?: number;
  /** Additional styles */
  style?: ViewStyle;
  /** Index for staggered animations (adds delay * index) */
  index?: number;
  /** Stagger delay per item (ms) */
  staggerDelay?: number;
  /** Whether card is pressable */
  pressable?: boolean;
  /** Scale on press (default: 0.98) */
  pressScale?: number;
}

/**
 * Animated card component with fade-in and slide-up entrance animation.
 * Supports press feedback and staggered animations for lists.
 *
 * @example
 * // Basic usage
 * <AnimatedCard>
 *   <Text>Card content</Text>
 * </AnimatedCard>
 *
 * // Pressable with callback
 * <AnimatedCard pressable onPress={() => navigate()}>
 *   <Text>Tap me</Text>
 * </AnimatedCard>
 *
 * // Staggered list animation
 * {items.map((item, index) => (
 *   <AnimatedCard key={item.id} index={index} staggerDelay={100}>
 *     <ItemContent item={item} />
 *   </AnimatedCard>
 * ))}
 */
export function AnimatedCard({
  children,
  delay = 0,
  duration = 300,
  style,
  index = 0,
  staggerDelay = 50,
  pressable = false,
  pressScale = 0.98,
  onPress,
  ...pressableProps
}: AnimatedCardProps) {
  const fadeAnim = useRef(new Animated.Value(0)).current;
  const translateAnim = useRef(new Animated.Value(20)).current;
  const scaleAnim = useRef(new Animated.Value(1)).current;

  useEffect(() => {
    const totalDelay = delay + index * staggerDelay;

    Animated.parallel([
      Animated.timing(fadeAnim, {
        toValue: 1,
        duration,
        delay: totalDelay,
        useNativeDriver: true,
      }),
      Animated.timing(translateAnim, {
        toValue: 0,
        duration,
        delay: totalDelay,
        useNativeDriver: true,
      }),
    ]).start();
  }, [fadeAnim, translateAnim, delay, duration, index, staggerDelay]);

  const handlePressIn = () => {
    if (!pressable) return;
    Animated.spring(scaleAnim, {
      toValue: pressScale,
      useNativeDriver: true,
      speed: 50,
      bounciness: 4,
    }).start();
  };

  const handlePressOut = () => {
    if (!pressable) return;
    Animated.spring(scaleAnim, {
      toValue: 1,
      useNativeDriver: true,
      speed: 50,
      bounciness: 4,
    }).start();
  };

  const animatedStyle = {
    opacity: fadeAnim,
    transform: [{ translateY: translateAnim }, { scale: scaleAnim }],
  };

  if (pressable) {
    return (
      <Pressable
        onPress={onPress}
        onPressIn={handlePressIn}
        onPressOut={handlePressOut}
        {...pressableProps}
      >
        <Animated.View style={[animatedStyle, style]}>{children}</Animated.View>
      </Pressable>
    );
  }

  return (
    <Animated.View style={[animatedStyle, style]}>{children}</Animated.View>
  );
}

/**
 * Animated fade-in wrapper for any content.
 * Simpler than AnimatedCard, just fade + optional scale.
 */
interface FadeInProps {
  children: React.ReactNode;
  delay?: number;
  duration?: number;
  style?: ViewStyle;
}

export function FadeIn({
  children,
  delay = 0,
  duration = 300,
  style,
}: FadeInProps) {
  const fadeAnim = useRef(new Animated.Value(0)).current;

  useEffect(() => {
    Animated.timing(fadeAnim, {
      toValue: 1,
      duration,
      delay,
      useNativeDriver: true,
    }).start();
  }, [fadeAnim, delay, duration]);

  return (
    <Animated.View style={[{ opacity: fadeAnim }, style]}>
      {children}
    </Animated.View>
  );
}

/**
 * Animated scale bounce effect for success/completion states.
 */
interface BounceInProps {
  children: React.ReactNode;
  delay?: number;
  style?: ViewStyle;
}

export function BounceIn({ children, delay = 0, style }: BounceInProps) {
  const scaleAnim = useRef(new Animated.Value(0)).current;

  useEffect(() => {
    Animated.spring(scaleAnim, {
      toValue: 1,
      delay,
      useNativeDriver: true,
      speed: 8,
      bounciness: 12,
    }).start();
  }, [scaleAnim, delay]);

  return (
    <Animated.View style={[{ transform: [{ scale: scaleAnim }] }, style]}>
      {children}
    </Animated.View>
  );
}

export default AnimatedCard;

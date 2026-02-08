/**
 * Tests for ErrorBoundary Component
 * Phase 5: Error Handling - Production Resilience
 *
 * ErrorBoundary catches JavaScript errors in child components,
 * logs errors, and displays a fallback UI.
 */
import React from "react";
import { Text, View, TouchableOpacity } from "react-native";

// Suppress console.error during error boundary tests
const originalError = console.error;
beforeAll(() => {
  console.error = jest.fn();
});
afterAll(() => {
  console.error = originalError;
});

// Component that throws an error for testing
const ThrowError: React.FC<{ shouldThrow?: boolean }> = ({ shouldThrow }) => {
  if (shouldThrow) {
    throw new Error("Test error");
  }
  return <Text>Child Content</Text>;
};

describe("ErrorBoundary", () => {
  describe("module exports", () => {
    it("exports ErrorBoundary component", () => {
      const ErrorBoundaryModule = require("../../components/ErrorBoundary");
      expect(ErrorBoundaryModule.ErrorBoundary).toBeDefined();
      expect(typeof ErrorBoundaryModule.ErrorBoundary).toBe("function");
    });

    it("exports as default", () => {
      const ErrorBoundaryModule = require("../../components/ErrorBoundary");
      expect(ErrorBoundaryModule.default).toBeDefined();
      expect(ErrorBoundaryModule.default).toBe(
        ErrorBoundaryModule.ErrorBoundary
      );
    });
  });

  describe("normal rendering", () => {
    it("renders children when no error occurs", () => {
      const renderer = require("react-test-renderer");
      const { ErrorBoundary } = require("../../components/ErrorBoundary");

      const tree = renderer.create(
        <ErrorBoundary>
          <Text>Normal Content</Text>
        </ErrorBoundary>
      );

      expect(tree.toJSON()).toBeTruthy();
      const jsonTree = JSON.stringify(tree.toJSON());
      expect(jsonTree).toContain("Normal Content");
    });
  });

  describe("error handling", () => {
    it("catches errors in child components", () => {
      const renderer = require("react-test-renderer");
      const { ErrorBoundary } = require("../../components/ErrorBoundary");

      // Should not throw - error boundary catches it
      expect(() => {
        renderer.create(
          <ErrorBoundary>
            <ThrowError shouldThrow={true} />
          </ErrorBoundary>
        );
      }).not.toThrow();
    });

    it("renders fallback UI when error occurs", () => {
      const renderer = require("react-test-renderer");
      const { ErrorBoundary } = require("../../components/ErrorBoundary");

      const tree = renderer.create(
        <ErrorBoundary>
          <ThrowError shouldThrow={true} />
        </ErrorBoundary>
      );

      const jsonTree = JSON.stringify(tree.toJSON());
      expect(jsonTree).toContain("Something went wrong");
    });

    it("displays error message in fallback", () => {
      const renderer = require("react-test-renderer");
      const { ErrorBoundary } = require("../../components/ErrorBoundary");

      const tree = renderer.create(
        <ErrorBoundary>
          <ThrowError shouldThrow={true} />
        </ErrorBoundary>
      );

      const jsonTree = JSON.stringify(tree.toJSON());
      expect(jsonTree).toContain("Test error");
    });
  });

  describe("retry functionality", () => {
    it("provides a retry button in fallback UI", () => {
      const renderer = require("react-test-renderer");
      const { ErrorBoundary } = require("../../components/ErrorBoundary");

      const tree = renderer.create(
        <ErrorBoundary>
          <ThrowError shouldThrow={true} />
        </ErrorBoundary>
      );

      const jsonTree = JSON.stringify(tree.toJSON());
      expect(jsonTree).toContain("Try Again");
    });

    it("calls onRetry prop when retry button pressed", () => {
      const renderer = require("react-test-renderer");
      const { ErrorBoundary } = require("../../components/ErrorBoundary");
      const onRetry = jest.fn();

      const tree = renderer.create(
        <ErrorBoundary onRetry={onRetry}>
          <ThrowError shouldThrow={true} />
        </ErrorBoundary>
      );

      // Find and press the retry button
      const root = tree.root;
      const retryButton = root.findByType(TouchableOpacity);
      retryButton.props.onPress();

      expect(onRetry).toHaveBeenCalledTimes(1);
    });
  });

  describe("custom fallback", () => {
    it("renders custom fallback when provided", () => {
      const renderer = require("react-test-renderer");
      const { ErrorBoundary } = require("../../components/ErrorBoundary");

      const CustomFallback = ({ error }: { error: Error }) => (
        <View>
          <Text>Custom Error: {error.message}</Text>
        </View>
      );

      const tree = renderer.create(
        <ErrorBoundary fallback={CustomFallback}>
          <ThrowError shouldThrow={true} />
        </ErrorBoundary>
      );

      const jsonTree = JSON.stringify(tree.toJSON());
      // React splits text nodes, so check for both parts
      expect(jsonTree).toContain("Custom Error:");
      expect(jsonTree).toContain("Test error");
    });
  });

  describe("index.ts exports", () => {
    it("re-exports ErrorBoundary from components/index.ts", () => {
      const Components = require("../../components");
      expect(Components.ErrorBoundary).toBeDefined();
      expect(typeof Components.ErrorBoundary).toBe("function");
    });
  });
});

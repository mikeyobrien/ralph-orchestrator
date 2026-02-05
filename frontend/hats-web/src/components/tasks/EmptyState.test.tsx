/**
 * EmptyState Component Tests
 *
 * Tests for the reusable EmptyState component that displays
 * centered empty state messages with icon, title, and description.
 */

import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { Inbox, Search, FileText } from "lucide-react";
import { EmptyState } from "./EmptyState";

describe("EmptyState", () => {
  describe("rendering", () => {
    it("renders with icon, title, and description", () => {
      render(
        <EmptyState
          icon={Inbox}
          title="No tasks yet"
          description="Give Hats something to do!"
        />
      );

      // Should render the title
      expect(screen.getByText("No tasks yet")).toBeInTheDocument();

      // Should render the description
      expect(screen.getByText("Give Hats something to do!")).toBeInTheDocument();

      // Should render the icon (Lucide adds class based on icon name)
      expect(document.querySelector(".lucide-inbox")).toBeInTheDocument();
    });

    it("renders with different icons", () => {
      const { rerender } = render(
        <EmptyState
          icon={Search}
          title="No results"
          description="Try a different search"
        />
      );

      expect(document.querySelector(".lucide-search")).toBeInTheDocument();

      rerender(
        <EmptyState
          icon={FileText}
          title="No files"
          description="Upload some files"
        />
      );

      expect(document.querySelector(".lucide-file-text")).toBeInTheDocument();
    });

    it("renders title as heading", () => {
      render(
        <EmptyState
          icon={Inbox}
          title="Empty State Title"
          description="Some description"
        />
      );

      // Title should be a paragraph or heading with appropriate styling
      const title = screen.getByText("Empty State Title");
      expect(title).toBeInTheDocument();
    });
  });

  describe("styling", () => {
    it("centers content", () => {
      render(
        <EmptyState
          icon={Inbox}
          title="Test"
          description="Description"
        />
      );

      // Container should have centering classes
      const container = screen.getByText("Test").closest("div");
      expect(container?.closest("[class*='flex']")).toHaveClass("flex-col");
      expect(container?.closest("[class*='items-center']")).toBeInTheDocument();
      expect(container?.closest("[class*='justify-center']")).toBeInTheDocument();
    });

    it("uses muted styling for text", () => {
      render(
        <EmptyState
          icon={Inbox}
          title="Test Title"
          description="Test description"
        />
      );

      // Description should have muted styling
      const description = screen.getByText("Test description");
      expect(description).toHaveClass("text-muted-foreground");
    });

    it("applies custom className", () => {
      render(
        <EmptyState
          icon={Inbox}
          title="Test"
          description="Desc"
          className="custom-class"
        />
      );

      // Should pass through className to container
      const container = document.querySelector(".custom-class");
      expect(container).toBeInTheDocument();
    });
  });

  describe("optional children", () => {
    it("renders children when provided", () => {
      render(
        <EmptyState
          icon={Inbox}
          title="Test"
          description="Desc"
        >
          <button>Action Button</button>
        </EmptyState>
      );

      expect(screen.getByRole("button", { name: "Action Button" })).toBeInTheDocument();
    });
  });
});

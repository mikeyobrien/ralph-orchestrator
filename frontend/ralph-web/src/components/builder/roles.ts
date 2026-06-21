/**
 * Shared role metadata for hat nodes.
 *
 * Consumed by HatNode (border + emoji) and CollectionBuilder's MiniMap
 * (node color). Single source of truth so the two views never drift.
 */

export interface RoleMeta {
  emoji: string;
  borderClass: string;
  color: string;
}

/**
 * Map of role key → visual metadata. Keys match the values used by
 * HatPalette's HAT_TEMPLATES (planner, builder, reviewer, validator,
 * confessor, custom). Unknown roles fall back to the "custom" entry.
 */
export const ROLE_META: Record<string, RoleMeta> = {
  planner: { emoji: "📋", borderClass: "border-violet-500/60", color: "#8b5cf6" },
  builder: { emoji: "⚡", borderClass: "border-blue-500/60", color: "#3b82f6" },
  reviewer: { emoji: "👀", borderClass: "border-green-500/60", color: "#22c55e" },
  validator: { emoji: "✅", borderClass: "border-amber-500/60", color: "#f59e0b" },
  confessor: { emoji: "🔍", borderClass: "border-red-500/60", color: "#ef4444" },
  custom: { emoji: "🎩", borderClass: "", color: "#6b7280" },
};

const UUID_SUFFIX_RE = /-[0-9a-f]{8}$/i;

/**
 * Extract the role from a node key. Nodes dropped from the palette receive
 * IDs like `planner-a1b2c3d4`; strip the uuid suffix to recover "planner".
 * If no suffix is present, the key is used as-is.
 */
export function getRole(key: string): string {
  return key.replace(UUID_SUFFIX_RE, "");
}

/**
 * Look up role metadata with a fallback to the "custom" entry.
 */
export function getRoleMeta(key: string): RoleMeta {
  const role = getRole(key);
  return ROLE_META[role] ?? ROLE_META.custom;
}

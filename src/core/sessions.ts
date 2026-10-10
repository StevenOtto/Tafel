/**
 * Sessions as the control window shows them. The rules (which session is
 * current, adoption on naming, expiry) live in Rust; this side only labels,
 * orders and formats.
 */

export interface Session {
  readonly id: string;
  /** Empty while provisional. */
  readonly name: string;
  readonly provisional: boolean;
  readonly createdMs: number;
  readonly lastDeliveryMs: number | null;
  readonly deliveryCount: number;
}

const startedAt = (ms: number): string =>
  new Date(ms).toLocaleString("en-GB", {
    weekday: "short",
    day: "numeric",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  });

/** A provisional session is labelled by when it started. */
export const sessionLabel = (session: Session): string =>
  session.provisional
    ? `Unnamed · ${startedAt(session.createdMs)}`
    : session.name;

const lastActivity = (session: Session): number =>
  session.lastDeliveryMs ?? session.createdMs;

/** Most recently active first, so the switcher leads with today's work. */
export const byRecentActivity = (sessions: readonly Session[]): Session[] =>
  [...sessions].sort((a, b) => lastActivity(b) - lastActivity(a));

export const formatBytes = (bytes: number): string => {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${Math.round(bytes / 1024)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
};

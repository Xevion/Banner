/**
 * useStream composable for type-safe WebSocket stream subscriptions.
 *
 * Provides declarative event handling with Svelte 5 runes.
 */
import {
  acquireStreamClient,
  releaseStreamClient,
  type ConnectionState,
  type FilterFor,
  type SnapshotFor,
} from "$lib/ws";
import type {
  StreamKind,
  StreamSnapshot,
  StreamDelta,
  ScrapeJobEvent,
  AuditLogEntry,
  ScraperStatsResponse,
  SubjectSummary,
  TimeseriesPoint,
} from "$lib/bindings";

type EventFor<S extends StreamKind> = S extends "scrapeJobs"
  ? ScrapeJobEvent
  : S extends "auditLog"
    ? { entries: AuditLogEntry[] }
    : S extends "scraperStats"
      ? { stats: ScraperStatsResponse }
      : S extends "scraperTimeseries"
        ? { changed: TimeseriesPoint[] }
        : S extends "scraperSubjects"
          ? { changed: SubjectSummary[]; removed: string[] }
          : never;

// Extract the event type discriminator
type EventType<S extends StreamKind> = EventFor<S> extends { type: infer T } ? T : never;

interface UseStreamOptions<S extends StreamKind, T> {
  /** Initial state before any data is received */
  initial: T;
  /** Event handlers keyed by event type (optional for computed streams) */
  on?: {
    [K in EventType<S> & string]?: (state: T, event: Extract<EventFor<S>, { type: K }>) => T;
  };
  /** Custom snapshot handler */
  onSnapshot?: (snapshot: SnapshotFor<S>) => T;
  /** Delta handler for computed streams or fallback */
  onDelta?: (state: T, event: EventFor<S>) => T;
}

interface UseStreamReturn<S extends StreamKind, T> {
  /** Current state (reactive) */
  readonly state: T;
  /** Current connection state (reactive) */
  readonly connectionState: ConnectionState;
  /** Modify the stream filter */
  modify: (newFilter: FilterFor<S>) => void;
  /** Retry connection after failure */
  retry: () => void;
}

/**
 * Create a reactive stream subscription with declarative event handlers.
 *
 * @example
 * ```svelte
 * <script lang="ts">
 *   const { state: jobs, connectionState } = useStream("scrapeJobs", null, {
 *     initial: [] as ScrapeJobDto[],
 *     on: {
 *       created: (jobs, e) => [...jobs, e.job],
 *       completed: (jobs, e) => jobs.filter(j => j.id !== e.id),
 *       locked: (jobs, e) => updateById(jobs, e.id, { status: e.status }),
 *     },
 *   });
 * </script>
 * ```
 */
export function useStream<S extends StreamKind, T>(
  stream: S,
  filter: FilterFor<S>,
  options: UseStreamOptions<S, T>
): UseStreamReturn<S, T> {
  let state = $state<T>(options.initial);
  let connectionState = $state<ConnectionState>("disconnected");
  let client: ReturnType<typeof acquireStreamClient> | null = null;
  let subscription: { modify: (f: FilterFor<S>) => void; unsubscribe: () => void } | null = null;

  $effect(() => {
    const stateChangeHandler = () => {
      connectionState = client!.getConnectionState();
    };
    client = acquireStreamClient(stateChangeHandler);

    subscription = client.subscribe(stream, filter, {
      onSnapshot: (snapshot) => {
        if (options.onSnapshot) {
          state = options.onSnapshot(snapshot);
        } else {
          state = extractSnapshotState(stream, snapshot) as T;
        }
      },
      onDelta: (delta) => {
        const event = extractEvent(stream, delta);
        if (!event) return;

        // For typed event streams (scrapeJobs), use the 'on' handlers
        if ("type" in event && options.on) {
          const eventType = event.type as EventType<S> & string;
          const handler = options.on[eventType];
          if (handler) {
            state = handler(state, event as never);
            return;
          }
        }

        // For computed streams or fallback, use onDelta
        if (options.onDelta) {
          state = options.onDelta(state, event);
        }
      },
    });

    return () => {
      subscription?.unsubscribe();
      releaseStreamClient(stateChangeHandler);
    };
  });

  return {
    get state() {
      return state;
    },
    get connectionState() {
      return connectionState;
    },
    modify: (newFilter: FilterFor<S>) => subscription?.modify(newFilter),
    retry: () => client?.retry(),
  };
}

/**
 * Extract the state data from a snapshot based on stream type.
 */
function extractSnapshotState<S extends StreamKind>(stream: S, snapshot: StreamSnapshot): unknown {
  if (stream === "scrapeJobs" && snapshot.stream === "scrapeJobs") {
    return snapshot.jobs;
  }
  if (stream === "auditLog" && snapshot.stream === "auditLog") {
    return snapshot.entries;
  }
  if (stream === "scraperStats" && snapshot.stream === "scraperStats") {
    return snapshot.stats;
  }
  if (stream === "scraperTimeseries" && snapshot.stream === "scraperTimeseries") {
    return { points: snapshot.points, period: snapshot.period, bucket: snapshot.bucket };
  }
  if (stream === "scraperSubjects" && snapshot.stream === "scraperSubjects") {
    return snapshot.subjects;
  }
  return null;
}

/**
 * Extract the event from a delta based on stream type.
 */
function extractEvent<S extends StreamKind>(stream: S, delta: StreamDelta): EventFor<S> | null {
  if (stream === "scrapeJobs" && delta.stream === "scrapeJobs") {
    return delta.event as EventFor<S>;
  }
  if (stream === "auditLog" && delta.stream === "auditLog") {
    return { entries: delta.entries } as EventFor<S>;
  }
  if (stream === "scraperStats" && delta.stream === "scraperStats") {
    return { stats: delta.stats } as EventFor<S>;
  }
  if (stream === "scraperTimeseries" && delta.stream === "scraperTimeseries") {
    return { changed: delta.changed } as EventFor<S>;
  }
  if (stream === "scraperSubjects" && delta.stream === "scraperSubjects") {
    return { changed: delta.changed, removed: delta.removed } as EventFor<S>;
  }
  return null;
}

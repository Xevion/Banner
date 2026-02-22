import type {
  StreamKind,
  StreamFilter,
  StreamClientMessage,
  StreamDelta,
  StreamServerMessage,
  StreamSnapshot,
  AuditLogFilter,
  ScrapeJobsFilter,
  ScraperStatsFilter,
  ScraperTimeseriesFilter,
} from "$lib/bindings";

export type ConnectionState = "connected" | "reconnecting" | "disconnected";

const MAX_RECONNECT_DELAY = 30_000;
const MAX_RECONNECT_ATTEMPTS = 10;

export type StreamKey = StreamKind;
export type SnapshotFor<S extends StreamKey> = Extract<StreamSnapshot, { stream: S }>;
type DeltaFor<S extends StreamKey> = Extract<StreamDelta, { stream: S }>;
export type FilterFor<S extends StreamKey> = S extends "scrapeJobs"
  ? ScrapeJobsFilter | null
  : S extends "auditLog"
    ? AuditLogFilter | null
    : S extends "scraperStats"
      ? ScraperStatsFilter
      : S extends "scraperTimeseries"
        ? ScraperTimeseriesFilter
        : S extends "scraperSubjects"
          ? null
          : never;

interface SubscriptionHandlers<S extends StreamKey> {
  onSnapshot: (snapshot: SnapshotFor<S>) => void;
  onDelta: (delta: DeltaFor<S>) => void;
  onError?: (error: StreamServerMessage & { type: "error" }) => void;
}

interface SubscriptionSpec<S extends StreamKey> {
  stream: S;
  filter: FilterFor<S>;
  handlers: SubscriptionHandlers<S>;
  subId: string | null;
}

interface PendingRequest {
  kind: "subscribe" | "modify" | "unsubscribe";
  spec?: SubscriptionSpec<StreamKey>;
}

export class StreamClient {
  private ws: WebSocket | null = null;
  private _connectionState: ConnectionState = "disconnected";
  private reconnectAttempts = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private intentionalClose = false;
  private requestSeq = 1;

  private subscriptions: SubscriptionSpec<StreamKey>[] = [];
  private subById = new Map<string, SubscriptionSpec<StreamKey>>();
  private pendingRequests = new Map<string, PendingRequest>();
  private stateListeners = new Set<() => void>();

  constructor(onStateChange?: () => void) {
    if (onStateChange) this.stateListeners.add(onStateChange);
  }

  addStateListener(fn: () => void): void {
    this.stateListeners.add(fn);
  }

  removeStateListener(fn: () => void): void {
    this.stateListeners.delete(fn);
  }

  private notifyStateListeners(): void {
    for (const fn of this.stateListeners) fn();
  }

  getConnectionState(): ConnectionState {
    return this._connectionState;
  }

  private closeExisting(): void {
    if (this.ws) {
      this.ws.onopen = null;
      this.ws.onmessage = null;
      this.ws.onclose = null;
      this.ws.onerror = null;
      this.ws.close();
      this.ws = null;
    }
  }

  connect(): void {
    this.closeExisting();
    this.intentionalClose = false;
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${protocol}//${window.location.host}/api/ws`;

    try {
      this.ws = new WebSocket(url);
    } catch {
      this.scheduleReconnect();
      return;
    }

    this.ws.onopen = () => {
      this._connectionState = "connected";
      this.reconnectAttempts = 0;
      this.subById.clear();
      this.pendingRequests.clear();
      for (const spec of this.subscriptions) {
        spec.subId = null;
        this.sendSubscribe(spec);
      }
      this.notifyStateListeners();
    };

    this.ws.onmessage = (event) => {
      try {
        const parsed = JSON.parse(event.data as string) as StreamServerMessage;
        this.handleMessage(parsed);
      } catch {
        // ignore malformed messages
      }
    };

    this.ws.onclose = () => {
      this.ws = null;
      if (!this.intentionalClose) {
        this.scheduleReconnect();
      }
    };

    this.ws.onerror = () => {
      // onclose will handle reconnects
    };
  }

  disconnect(): void {
    this.intentionalClose = true;
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.closeExisting();
    this._connectionState = "disconnected";
    this.notifyStateListeners();
  }

  retry(): void {
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.closeExisting();
    this.reconnectAttempts = 0;
    this._connectionState = "reconnecting";
    this.notifyStateListeners();
    this.connect();
  }

  subscribe<S extends StreamKey>(
    stream: S,
    filter: FilterFor<S>,
    handlers: SubscriptionHandlers<S>
  ): { modify: (next: FilterFor<S>) => void; unsubscribe: () => void } {
    const spec: SubscriptionSpec<S> = {
      stream,
      filter,
      handlers,
      subId: null,
    };
    const storedSpec = spec as unknown as SubscriptionSpec<StreamKey>;

    this.subscriptions.push(storedSpec);
    this.sendSubscribe(storedSpec);

    return {
      modify: (next) => {
        spec.filter = next;
        if (spec.subId) {
          this.sendModify(spec.subId, spec.stream, next);
        }
      },
      unsubscribe: () => {
        this.subscriptions = this.subscriptions.filter((s) => s !== storedSpec);
        if (spec.subId) {
          this.sendUnsubscribe(spec.subId);
          this.subById.delete(spec.subId);
          spec.subId = null;
        }
      },
    };
  }

  private handleMessage(message: StreamServerMessage): void {
    switch (message.type) {
      case "ready":
        return;
      case "subscribed": {
        const pending = this.pendingRequests.get(message.request_id);
        if (!pending?.spec) return;
        pending.spec.subId = message.subscription_id;
        this.subById.set(message.subscription_id, pending.spec);
        this.pendingRequests.delete(message.request_id);
        return;
      }
      case "modified":
      case "unsubscribed":
        this.pendingRequests.delete(message.request_id);
        return;
      case "snapshot": {
        const spec = this.subById.get(message.subscription_id);
        if (!spec) return;
        spec.handlers.onSnapshot(message.snapshot as SnapshotFor<StreamKey>);
        return;
      }
      case "delta": {
        const spec = this.subById.get(message.subscription_id);
        if (!spec) return;
        spec.handlers.onDelta(message.delta as DeltaFor<StreamKey>);
        return;
      }
      case "error": {
        const pending = message.request_id
          ? this.pendingRequests.get(message.request_id)
          : undefined;
        if (pending?.spec?.handlers.onError) {
          pending.spec.handlers.onError(message);
          if (message.request_id) this.pendingRequests.delete(message.request_id);
        }
        return;
      }
      case "pong":
        return;
    }
  }

  private sendSubscribe(spec: SubscriptionSpec<StreamKey>): void {
    const requestId = this.nextRequestId();
    const filter =
      spec.filter === null
        ? null
        : spec.filter
          ? ({ stream: spec.stream, ...spec.filter } as StreamFilter)
          : undefined;
    const message: StreamClientMessage = {
      type: "subscribe",
      request_id: requestId,
      stream: spec.stream,
      filter,
    };
    if (this.sendMessage(message)) {
      this.pendingRequests.set(requestId, { kind: "subscribe", spec });
    }
  }

  private sendModify<S extends StreamKey>(subId: string, stream: S, filter: FilterFor<S>): void {
    const requestId = this.nextRequestId();
    const wrappedFilter =
      filter === null ? null : filter ? ({ stream, ...filter } as StreamFilter) : undefined;
    const message: StreamClientMessage = {
      type: "modify",
      request_id: requestId,
      subscription_id: subId,
      filter: wrappedFilter,
    };
    if (this.sendMessage(message)) {
      this.pendingRequests.set(requestId, { kind: "modify" });
    }
  }

  private sendUnsubscribe(subId: string): void {
    const requestId = this.nextRequestId();
    const message: StreamClientMessage = {
      type: "unsubscribe",
      request_id: requestId,
      subscription_id: subId,
    };
    if (this.sendMessage(message)) {
      this.pendingRequests.set(requestId, { kind: "unsubscribe" });
    }
  }

  private sendMessage(message: StreamClientMessage): boolean {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
      return true;
    }
    return false;
  }

  private scheduleReconnect(): void {
    if (this.reconnectAttempts >= MAX_RECONNECT_ATTEMPTS) {
      this._connectionState = "disconnected";
      this.notifyStateListeners();
      return;
    }

    this._connectionState = "reconnecting";
    this.notifyStateListeners();

    const delay = Math.min(1000 * 2 ** this.reconnectAttempts, MAX_RECONNECT_DELAY);
    this.reconnectAttempts++;

    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, delay);
  }

  private nextRequestId(): string {
    // TODO: Switch to something shorter/simpler like nanoid.
    const id = this.requestSeq;
    this.requestSeq += 1;
    return `req_${Date.now()}_${id}`;
  }
}

// Shared singleton for multiple useStream calls on the same page
let sharedClient: StreamClient | null = null;
let refCount = 0;

export function acquireStreamClient(onStateChange: () => void): StreamClient {
  if (!sharedClient) {
    sharedClient = new StreamClient();
    sharedClient.connect();
  }
  refCount++;
  sharedClient.addStateListener(onStateChange);
  return sharedClient;
}

export function releaseStreamClient(onStateChange: () => void): void {
  if (!sharedClient) return;
  sharedClient.removeStateListener(onStateChange);
  refCount--;
  if (refCount <= 0) {
    sharedClient.disconnect();
    sharedClient = null;
    refCount = 0;
  }
}

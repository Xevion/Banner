<script lang="ts">
import { client } from "$lib/api";
import type { ServiceInfo, ServiceStatus, StatusResponse } from "$lib/bindings";
import Footer from "$lib/components/Footer.svelte";
import SimpleTooltip from "$lib/components/SimpleTooltip.svelte";
import { relativeTime } from "$lib/time";
import { formatNumber } from "$lib/utils";
import {
  ArrowLeft,
  Bot,
  Clock,
  Database,
  Globe,
  Hourglass,
  MessageCircle,
  RefreshCw,
  Server,
} from "@lucide/svelte";
import { onMount } from "svelte";
import type { PageData } from "./$types";

let { data }: { data: PageData } = $props();

const REFRESH_INTERVAL = import.meta.env.DEV ? 3000 : 30000;
const REQUEST_TIMEOUT = 10000;

const SERVICE_ICONS: Record<string, typeof Bot> = {
  bot: Bot,
  banner: Globe,
  discord: MessageCircle,
  database: Database,
  web: Server,
  scraper: Clock,
};

const STATUS_COLORS: Record<ServiceStatus | "Unreachable", string> = {
  active: "var(--status-green)",
  connected: "var(--status-green)",
  starting: "var(--status-orange)",
  disabled: "var(--status-gray)",
  error: "var(--status-red)",
  Unreachable: "var(--status-red)",
};

function formatStatus(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

function toTitleCase(s: string): string {
  return s.replace(/\b\w/g, (c) => c.toUpperCase());
}

const OVERALL_STATUS_LABELS: Record<ServiceStatus | "Unreachable", string> = {
  active: "All Good",
  connected: "All Good",
  starting: "Starting Up",
  disabled: "Degraded",
  error: "Issues",
  Unreachable: "Offline",
};

interface ResponseTiming {
  health: number | null;
  status: number | null;
}

interface Service {
  name: string;
  status: ServiceStatus;
  icon: typeof Bot;
}

type StatusState =
  | { mode: "loading" }
  | { mode: "response"; timing: ResponseTiming; lastFetch: Date; status: StatusResponse }
  | { mode: "error"; lastFetch: Date }
  | { mode: "timeout"; lastFetch: Date };

// svelte-ignore state_referenced_locally
let statusState = $state<StatusState>(
  data.initialStatus
    ? {
        mode: "response",
        status: data.initialStatus,
        timing: { health: null, status: null },
        lastFetch: new Date(),
      }
    : { mode: "loading" }
);
let now = $state(new Date());
let isRefreshing = $state(false);

// Module-level fetch coordination -- safe since this is a single-instance page
let _scheduledTimeoutId: ReturnType<typeof setTimeout> | null = null;
let _requestTimeoutId: ReturnType<typeof setTimeout> | null = null;
let _isFetching = false;
let _cancelled = false;

const isLoading = $derived(statusState.mode === "loading");
const shouldShowSkeleton = $derived(statusState.mode === "loading" || statusState.mode === "error");

const overallHealth: ServiceStatus | "Unreachable" = $derived(
  statusState.mode === "timeout"
    ? "Unreachable"
    : statusState.mode === "error"
      ? "error"
      : statusState.mode === "response"
        ? statusState.status.status
        : "error"
);

const overallColor = $derived(STATUS_COLORS[overallHealth]);
const isHealthy = $derived(overallHealth === "active" || overallHealth === "connected");

const services: Service[] = $derived(
  statusState.mode === "response"
    ? (Object.entries(statusState.status.services) as [string, ServiceInfo][]).map(
        ([id, info]) => ({
          name: info.name,
          status: info.status,
          icon: SERVICE_ICONS[id] ?? Bot,
        })
      )
    : []
);

const shouldShowTiming = $derived(
  statusState.mode === "response" && statusState.timing.health !== null
);

const shouldShowLastFetch = $derived(
  statusState.mode === "response" || statusState.mode === "error" || statusState.mode === "timeout"
);

const lastFetch = $derived(
  statusState.mode === "response" || statusState.mode === "error" || statusState.mode === "timeout"
    ? statusState.lastFetch
    : null
);

const relativeLastFetchResult = $derived(lastFetch ? relativeTime(lastFetch, now) : null);
const relativeLastFetch = $derived(relativeLastFetchResult?.text ?? "");

async function doFetch() {
  if (_isFetching) return;
  _isFetching = true;

  try {
    const startTime = Date.now();

    const timeoutPromise = new Promise<never>((_, reject) => {
      _requestTimeoutId = setTimeout(() => {
        reject(new Error("Request timeout"));
      }, REQUEST_TIMEOUT);
    });

    const result = await Promise.race([client.getStatus(), timeoutPromise]);

    if (_requestTimeoutId) {
      clearTimeout(_requestTimeoutId);
      _requestTimeoutId = null;
    }

    if (result.isErr) {
      statusState = { mode: "error", lastFetch: new Date() };
    } else {
      const responseTime = Date.now() - startTime;
      statusState = {
        mode: "response",
        status: result.value,
        timing: { health: responseTime, status: responseTime },
        lastFetch: new Date(),
      };
    }
  } catch (err) {
    if (_requestTimeoutId) {
      clearTimeout(_requestTimeoutId);
      _requestTimeoutId = null;
    }

    const message = err instanceof Error ? err.message : "";

    if (message === "Request timeout") {
      statusState = { mode: "timeout", lastFetch: new Date() };
    } else {
      statusState = { mode: "error", lastFetch: new Date() };
    }
  } finally {
    _isFetching = false;
    if (!_cancelled) {
      _scheduledTimeoutId = setTimeout(() => void doFetch(), REFRESH_INTERVAL);
    }
  }
}

function triggerRefresh() {
  if (isRefreshing || _isFetching) return;
  if (_scheduledTimeoutId) {
    clearTimeout(_scheduledTimeoutId);
    _scheduledTimeoutId = null;
  }
  isRefreshing = true;
  void doFetch().then(() => {
    setTimeout(() => {
      isRefreshing = false;
    }, 400);
  });
}

onMount(() => {
  let nowTimeoutId: ReturnType<typeof setTimeout> | null = null;

  // Adaptive tick: schedules the next `now` update based on when the
  // relative time text would actually change (every ~1s for recent
  // timestamps, every ~1m for minute-level, etc.)
  function scheduleNowTick() {
    const delay = relativeLastFetchResult?.nextUpdateMs ?? 1000;
    nowTimeoutId = setTimeout(() => {
      now = new Date();
      scheduleNowTick();
    }, delay);
  }
  scheduleNowTick();

  _cancelled = false;

  // If we have data from the load function, schedule the next refresh
  // instead of fetching immediately
  if (statusState.mode === "response") {
    _scheduledTimeoutId = setTimeout(() => void doFetch(), REFRESH_INTERVAL);
  } else {
    void doFetch();
  }

  return () => {
    _cancelled = true;
    if (_scheduledTimeoutId) clearTimeout(_scheduledTimeoutId);
    if (_requestTimeoutId) clearTimeout(_requestTimeoutId);
    if (nowTimeoutId) clearTimeout(nowTimeoutId);
  };
});
</script>

<svelte:head>
  <title>System Status | Banner</title>
</svelte:head>

<div class="min-h-screen flex flex-col items-center pt-24 px-5 pb-8">
  <!-- Page header row: back link -->
  <div class="w-full max-w-lg mb-4">
    <a
      href="/"
      class="flex items-center gap-1.5 text-sm text-muted-foreground hover:text-foreground no-underline transition-colors"
    >
      <ArrowLeft size={14} />
      Back
    </a>
  </div>

  <!-- Main card -->
  <div
    class="bg-card text-card-foreground rounded-xl border border-border p-7 w-full max-w-lg shadow-sm"
  >
    <div class="flex flex-col gap-5">
      <!-- Overall Status -->
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2.5">
          <!-- Live status dot -->
          <span class="relative flex h-2.5 w-2.5 flex-shrink-0">
            {#if !isLoading && isHealthy}
              <span
                class="animate-ping absolute inline-flex h-full w-full rounded-full opacity-50"
                style="background-color: {overallColor}"
              ></span>
            {/if}
            <span
              class="relative inline-flex h-2.5 w-2.5 rounded-full transition-colors duration-500"
              class:animate-pulse={isLoading}
              style={isLoading
                ? "background-color: color-mix(in oklch, var(--muted-foreground) 30%, transparent)"
                : `background-color: ${overallColor}`}
            ></span>
          </span>
          <span class="text-base font-medium text-foreground">System Status</span>
        </div>
        {#if isLoading}
          <div class="h-5 w-20 bg-muted rounded-full animate-pulse"></div>
        {:else}
          <span
            class="rounded-full px-2.5 py-0.5 text-xs font-medium"
            style="background-color: color-mix(in oklch, {overallColor} 15%, transparent); color: {overallColor}"
          >
            {OVERALL_STATUS_LABELS[overallHealth]}
          </span>
        {/if}
      </div>

      <!-- Services -->
      <div class="rounded-lg border border-border overflow-hidden">
        {#if shouldShowSkeleton}
          {#each Array(3) as _, i (i)}
            <div
              class="flex items-center justify-between px-4 py-3 border-b border-border last:border-b-0"
            >
              <div class="flex items-center gap-2.5">
                <div class="h-4 w-4 bg-muted rounded animate-pulse"></div>
                <div class="h-4 w-20 bg-muted rounded animate-pulse"></div>
              </div>
              <div class="h-5 w-16 bg-muted rounded-full animate-pulse"></div>
            </div>
          {/each}
        {:else}
          {#each services as service (service.name)}
            {@const color = STATUS_COLORS[service.status]}
            {@const ServiceIcon = service.icon}
            <div
              class="flex items-center justify-between px-4 py-3 border-b border-border last:border-b-0"
            >
              <div class="flex items-center gap-2.5 text-muted-foreground">
                <ServiceIcon size={15} />
                <span class="text-sm">{toTitleCase(service.name)}</span>
              </div>
              <span
                class="rounded-full px-2.5 py-0.5 text-xs font-medium"
                style="background-color: color-mix(in oklch, {color} 15%, transparent); color: {color}"
              >
                {formatStatus(service.status)}
              </span>
            </div>
          {/each}
        {/if}
      </div>

      <!-- Timing & Last Updated -->
      <div class="flex flex-col gap-2 pt-4 border-t border-border">
        {#if isLoading}
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2">
              <Hourglass size={13} class="text-muted-foreground" />
              <span class="text-sm text-muted-foreground">Response Time</span>
            </div>
            <div class="h-4 w-12 bg-muted rounded animate-pulse"></div>
          </div>
        {:else if shouldShowTiming && statusState.mode === "response"}
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2">
              <Hourglass size={13} class="text-muted-foreground" />
              <span class="text-sm text-muted-foreground">Response Time</span>
            </div>
            <span class="text-sm text-muted-foreground">
              {formatNumber(statusState.timing.health!)}ms
            </span>
          </div>
        {/if}

        {#if isLoading}
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2">
              <Clock size={13} class="text-muted-foreground" />
              <span class="text-sm text-muted-foreground">Last Updated</span>
            </div>
            <span class="text-sm text-muted-foreground">Loading...</span>
          </div>
        {:else if shouldShowLastFetch && lastFetch}
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2">
              <Clock size={13} class="text-muted-foreground" />
              <span class="text-sm text-muted-foreground">Last Updated</span>
            </div>
            <div class="flex items-center gap-2">
              <SimpleTooltip text="as of {lastFetch.toLocaleTimeString()}" delay={150} passthrough>
                <abbr
                  class="cursor-pointer underline decoration-dotted decoration-border underline-offset-[6px]"
                >
                  <span class="text-sm text-muted-foreground">{relativeLastFetch}</span>
                </abbr>
              </SimpleTooltip>
              <button
                onclick={triggerRefresh}
                class="text-muted-foreground hover:text-foreground transition-colors cursor-pointer bg-transparent border-none p-0 flex items-center"
                title="Refresh now"
                disabled={isRefreshing}
              >
                <RefreshCw size={13} class={isRefreshing ? "animate-spin" : ""} />
              </button>
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>

  <!-- Footer -->
  <Footer
    commitHash={statusState.mode === "response" ? statusState.status.commit : undefined}
    showStatusLink={false}
    class="mt-3 pt-0 pb-0"
  />
</div>

<script lang="ts">
import type { ServiceInfo, ServiceStatus, StatusResponse } from "$lib/bindings";
import { client } from "$lib/api";
import Footer from "$lib/components/Footer.svelte";
import SimpleTooltip from "$lib/components/SimpleTooltip.svelte";
import { relativeTime } from "$lib/time";
import { formatNumber } from "$lib/utils";
import {
  Activity,
  Bot,
  CheckCircle,
  Circle,
  Clock,
  Globe,
  Hourglass,
  MessageCircle,
  WifiOff,
  XCircle,
} from "@lucide/svelte";
import { onMount } from "svelte";

const REFRESH_INTERVAL = import.meta.env.DEV ? 3000 : 30000;
const REQUEST_TIMEOUT = 10000;

const SERVICE_ICONS: Record<string, typeof Bot> = {
  bot: Bot,
  banner: Globe,
  discord: MessageCircle,
  database: Activity,
  web: Globe,
  scraper: Clock,
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

const STATUS_ICONS: Record<
  ServiceStatus | "Unreachable",
  { icon: typeof CheckCircle; color: string }
> = {
  active: { icon: CheckCircle, color: "var(--status-green)" },
  connected: { icon: CheckCircle, color: "var(--status-green)" },
  starting: { icon: Hourglass, color: "var(--status-orange)" },
  disabled: { icon: Circle, color: "var(--status-gray)" },
  error: { icon: XCircle, color: "var(--status-red)" },
  Unreachable: { icon: WifiOff, color: "var(--status-red)" },
};

let statusState = $state({ mode: "loading" } as StatusState);
let now = $state(new Date());

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

const overallIcon = $derived(STATUS_ICONS[overallHealth]);

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

onMount(() => {
  let timeoutId: ReturnType<typeof setTimeout> | null = null;
  let requestTimeoutId: ReturnType<typeof setTimeout> | null = null;
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

  const fetchData = async () => {
    try {
      const startTime = Date.now();

      const timeoutPromise = new Promise<never>((_, reject) => {
        requestTimeoutId = setTimeout(() => {
          reject(new Error("Request timeout"));
        }, REQUEST_TIMEOUT);
      });

      const result = await Promise.race([client.getStatus(), timeoutPromise]);

      if (requestTimeoutId) {
        clearTimeout(requestTimeoutId);
        requestTimeoutId = null;
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
      if (requestTimeoutId) {
        clearTimeout(requestTimeoutId);
        requestTimeoutId = null;
      }

      const message = err instanceof Error ? err.message : "";

      if (message === "Request timeout") {
        statusState = { mode: "timeout", lastFetch: new Date() };
      } else {
        statusState = { mode: "error", lastFetch: new Date() };
      }
    }

    timeoutId = setTimeout(() => void fetchData(), REFRESH_INTERVAL);
  };

  void fetchData();

  return () => {
    if (timeoutId) clearTimeout(timeoutId);
    if (requestTimeoutId) clearTimeout(requestTimeoutId);
    if (nowTimeoutId) clearTimeout(nowTimeoutId);
  };
});
</script>

<svelte:head>
  <title>System Status | Banner</title>
</svelte:head>

<div class="min-h-screen flex flex-col items-center justify-center p-5">
  <div
    class="bg-card text-card-foreground rounded-xl border border-border p-6 w-full max-w-[400px] shadow-sm"
  >
    <div class="flex flex-col gap-4">
      <!-- Overall Status -->
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2">
          <Activity
            size={18}
            color={isLoading ? undefined : overallIcon.color}
            class={isLoading ? "animate-pulse" : ""}
            style="opacity: {isLoading ? 0.3 : 1}; transition: opacity 2s ease-in-out, color 2s ease-in-out;"
          />
          <span class="text-base font-medium text-foreground">System Status</span>
        </div>
        {#if isLoading}
          <div class="h-5 w-20 bg-muted rounded animate-pulse"></div>
        {:else}
          {#if overallIcon}
            {@const OverallIconComponent = overallIcon.icon}
            <div class="flex items-center gap-1.5">
              <span
                class="text-sm"
                class:text-muted-foreground={overallHealth === "disabled"}
                class:opacity-70={overallHealth === "disabled"}
              >
                {overallHealth}
              </span>
              <OverallIconComponent size={16} color={overallIcon.color} />
            </div>
          {/if}
        {/if}
      </div>

      <!-- Services -->
      <div class="flex flex-col gap-3 mt-4">
        {#if shouldShowSkeleton}
          {#each Array(3) as _service, i (i)}
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-2">
                <div class="h-6 w-[18px] bg-muted rounded animate-pulse"></div>
                <div class="h-6 w-[60px] bg-muted rounded animate-pulse"></div>
              </div>
              <div class="flex items-center gap-2">
                <div class="h-5 w-[50px] bg-muted rounded animate-pulse"></div>
                <div class="h-5 w-4 bg-muted rounded animate-pulse"></div>
              </div>
            </div>
          {/each}
        {:else}
          {#each services as service (service.name)}
            {@const statusInfo = STATUS_ICONS[service.status]}
            {@const ServiceIcon = service.icon}
            {@const StatusIconComponent = statusInfo.icon}
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-2">
                <ServiceIcon size={18} />
                <span class="text-muted-foreground">{service.name}</span>
              </div>
              <div class="flex items-center gap-1.5">
                <span
                  class="text-sm"
                  class:text-muted-foreground={service.status === "disabled"}
                  class:opacity-70={service.status === "disabled"}
                >
                  {service.status}
                </span>
                <StatusIconComponent size={16} color={statusInfo.color} />
              </div>
            </div>
          {/each}
        {/if}
      </div>

      <!-- Timing & Last Updated -->
      <div class="flex flex-col gap-2 mt-4 pt-4 border-t border-border">
        {#if isLoading}
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2">
              <Hourglass size={13} />
              <span class="text-sm text-muted-foreground">Response Time</span>
            </div>
            <div class="h-[18px] w-[50px] bg-muted rounded animate-pulse"></div>
          </div>
        {:else if shouldShowTiming && statusState.mode === "response"}
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2">
              <Hourglass size={13} />
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
              <Clock size={13} />
              <span class="text-sm text-muted-foreground">Last Updated</span>
            </div>
            <span class="text-sm text-muted-foreground pb-0.5">Loading...</span>
          </div>
        {:else if shouldShowLastFetch && lastFetch}
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2">
              <Clock size={13} />
              <span class="text-sm text-muted-foreground">Last Updated</span>
            </div>
            <SimpleTooltip text="as of {lastFetch.toLocaleTimeString()}" delay={150} passthrough>
              <abbr
                class="cursor-pointer underline decoration-dotted decoration-border underline-offset-[6px]"
              >
                <span class="text-sm text-muted-foreground">{relativeLastFetch}</span>
              </abbr>
            </SimpleTooltip>
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

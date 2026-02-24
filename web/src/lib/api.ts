import { authStore } from "$lib/auth.svelte";
import type {
  AdminStatusResponse,
  ApiError,
  ApiErrorCode,
  AssignBody,
  AuditLogResponse,
  BluebookLinkDetail,
  BluebookMatchResponse,
  BluebookOkResponse,
  BluebookSyncTriggerResponse,
  CodeDescription,
  CourseResponse,
  InstructorDetailResponse,
  ListBluebookLinksParams,
  ListBluebookLinksResponse,
  ListInstructorsParams as ListInstructorsParamsGenerated,
  ListInstructorsResponse,
  MatchBody,
  MetricsParams as MetricsParamsGenerated,
  MetricsResponse,
  PublicInstructorListResponse,
  PublicInstructorProfileResponse,
  RejectCandidateBody,
  RescoreResponse,
  ScrapeJobsResponse,
  ScraperStatsResponse,
  SearchOptionsResponse,
  SearchParams as SearchParamsGenerated,
  SearchResponse as SearchResponseGenerated,
  StatusResponse,
  SubjectDetailResponse,
  SubjectsResponse,
  SuggestResponse,
  TermResponse,
  TermSyncResponse,
  TermUpdateResponse,
  TermsListResponse,
  TimeRange,
  TimelineRequest,
  TimelineResponse,
  TimeseriesResponse,
  User,
} from "$lib/bindings";
import type Result from "true-myth/result";
import { err, ok } from "true-myth/result";

const API_BASE_URL = "/api";

// Semantic aliases
export type Term = TermResponse;
export type Subject = CodeDescription;
export type ReferenceEntry = CodeDescription;

// Re-export with simplified names
export type SearchResponse = SearchResponseGenerated;
export type SearchParams = SearchParamsGenerated;
export type MetricsParams = MetricsParamsGenerated;
export type ListInstructorsParams = ListInstructorsParamsGenerated;

export type ScraperPeriod = "1h" | "6h" | "24h" | "7d" | "30d";

/**
 * Converts a typed object to URLSearchParams, preserving camelCase keys.
 * Handles arrays, optional values, and primitives.
 */
function toURLSearchParams(obj: Record<string, unknown>): URLSearchParams {
  const params = new URLSearchParams();

  for (const [key, value] of Object.entries(obj)) {
    if (value === undefined || value === null) {
      continue; // Skip undefined/null values
    }

    if (Array.isArray(value)) {
      // Append each array element
      for (const item of value) {
        if (item !== undefined && item !== null) {
          params.append(key, String(item));
        }
      }
    } else if (typeof value === "object") {
      // JSON stringify objects
      params.set(key, JSON.stringify(value));
    } else {
      // Convert primitives to string (string, number, boolean, bigint, symbol)
      params.set(key, String(value as string | number | boolean));
    }
  }

  return params;
}

/**
 * API error class that wraps the structured ApiError response from the backend.
 */
export class ApiErrorClass extends Error {
  public readonly code: ApiErrorCode;
  public readonly details: unknown;

  constructor(apiError: ApiError) {
    super(apiError.message);
    this.name = "ApiError";
    this.code = apiError.code;
    this.details = apiError.details;
  }

  isNotFound(): boolean {
    return this.code === "NOT_FOUND";
  }

  isBadRequest(): boolean {
    return (
      this.code === "BAD_REQUEST" || this.code === "INVALID_TERM" || this.code === "INVALID_RANGE"
    );
  }

  isInternalError(): boolean {
    return this.code === "INTERNAL_ERROR";
  }
}

/** Module-level cache shared by all BannerApiClient instances. */
const _searchOptionsCache = new Map<string, { data: SearchOptionsResponse; fetchedAt: number }>();
const SEARCH_OPTIONS_TTL = 10 * 60 * 1000; // 10 minutes

export class BannerApiClient {
  private baseUrl: string;
  private fetchFn: typeof fetch;

  constructor(baseUrl: string = API_BASE_URL, fetchFn: typeof fetch = fetch) {
    this.baseUrl = baseUrl;
    this.fetchFn = fetchFn;
  }

  private buildInit(options?: { method?: string; body?: unknown }): RequestInit | undefined {
    if (!options) return undefined;
    const init: RequestInit = {};
    if (options.method) {
      init.method = options.method;
    }
    if (options.body !== undefined) {
      init.headers = { "Content-Type": "application/json" };
      init.body = JSON.stringify(options.body);
    } else if (options.method) {
      init.headers = { "Content-Type": "application/json" };
    }
    return Object.keys(init).length > 0 ? init : undefined;
  }

  private responseToErr(response: Response, apiError?: ApiError): Result<never, ApiErrorClass> {
    if (response.status === 401) {
      authStore.handleUnauthorized();
    }
    const error =
      apiError ??
      ({
        code: "INTERNAL_ERROR",
        message: `API request failed: ${response.status} ${response.statusText}`,
        details: null,
      } satisfies ApiError);
    return err(new ApiErrorClass(error));
  }

  private async request<T>(
    endpoint: string,
    options?: { method?: string; body?: unknown }
  ): Promise<Result<T, ApiErrorClass>> {
    const init = this.buildInit(options);
    const args: [string, RequestInit?] = [`${this.baseUrl}${endpoint}`];
    if (init) args.push(init);

    let response: Response;
    try {
      response = await this.fetchFn(...args);
    } catch (e) {
      return err(
        new ApiErrorClass({
          code: "INTERNAL_ERROR",
          message: e instanceof Error ? e.message : "Network request failed",
          details: null,
        })
      );
    }

    if (!response.ok) {
      let apiError: ApiError | undefined;
      try {
        apiError = (await response.json()) as ApiError;
      } catch {
        // Fall through — responseToErr uses a default
      }
      return this.responseToErr(response, apiError);
    }

    return ok((await response.json()) as T);
  }

  private async requestVoid(
    endpoint: string,
    options?: { method?: string; body?: unknown }
  ): Promise<Result<void, ApiErrorClass>> {
    const init = this.buildInit(options);
    const args: [string, RequestInit?] = [`${this.baseUrl}${endpoint}`];
    if (init) args.push(init);

    let response: Response;
    try {
      response = await this.fetchFn(...args);
    } catch (e) {
      return err(
        new ApiErrorClass({
          code: "INTERNAL_ERROR",
          message: e instanceof Error ? e.message : "Network request failed",
          details: null,
        })
      );
    }

    if (!response.ok) {
      let apiError: ApiError | undefined;
      try {
        apiError = (await response.json()) as ApiError;
      } catch {
        // Fall through — responseToErr uses a default
      }
      return this.responseToErr(response, apiError);
    }

    return ok(undefined as unknown as void);
  }

  async getStatus(): Promise<Result<StatusResponse, ApiErrorClass>> {
    return this.request<StatusResponse>("/status");
  }

  async searchCourses(
    params: Partial<SearchParams> & { term: string }
  ): Promise<Result<SearchResponse, ApiErrorClass>> {
    const query = toURLSearchParams(params as Record<string, unknown>);
    return this.request<SearchResponse>(`/courses/search?${query.toString()}`);
  }

  async getRelatedSections(
    term: string,
    subject: string,
    courseNumber: string
  ): Promise<Result<CourseResponse[], ApiErrorClass>> {
    return this.request<CourseResponse[]>(
      `/courses/${encodeURIComponent(term)}/${encodeURIComponent(subject)}/${encodeURIComponent(courseNumber)}/sections`
    );
  }

  async getTerms(): Promise<Result<Term[], ApiErrorClass>> {
    return this.request<Term[]>("/terms");
  }

  async getSubjects(term: string): Promise<Result<Subject[], ApiErrorClass>> {
    return this.request<Subject[]>(`/subjects?term=${encodeURIComponent(term)}`);
  }

  async getReference(category: string): Promise<Result<ReferenceEntry[], ApiErrorClass>> {
    return this.request<ReferenceEntry[]>(`/reference/${encodeURIComponent(category)}`);
  }

  async getSearchOptions(term?: string): Promise<Result<SearchOptionsResponse, ApiErrorClass>> {
    const cacheKey = term ?? "__default__";
    const cached = _searchOptionsCache.get(cacheKey);
    if (cached && Date.now() - cached.fetchedAt < SEARCH_OPTIONS_TTL) {
      return ok(cached.data);
    }
    const url = term ? `/search-options?term=${encodeURIComponent(term)}` : "/search-options";
    const result = await this.request<SearchOptionsResponse>(url);
    if (result.isOk) {
      _searchOptionsCache.set(cacheKey, { data: result.value, fetchedAt: Date.now() });
    }
    return result;
  }

  // Public instructor endpoints

  async getInstructors(params?: {
    search?: string;
    subject?: string;
    sort?: string;
    page?: number;
    perPage?: number;
  }): Promise<Result<PublicInstructorListResponse, ApiErrorClass>> {
    if (!params) {
      return this.request<PublicInstructorListResponse>("/instructors");
    }
    const query = toURLSearchParams(params as Record<string, unknown>);
    const qs = query.toString();
    return this.request<PublicInstructorListResponse>(`/instructors${qs ? `?${qs}` : ""}`);
  }

  async getInstructor(
    slug: string
  ): Promise<Result<PublicInstructorProfileResponse, ApiErrorClass>> {
    return this.request<PublicInstructorProfileResponse>(
      `/instructors/${encodeURIComponent(slug)}`
    );
  }

  async getInstructorSections(
    slug: string,
    term: string
  ): Promise<Result<CourseResponse[], ApiErrorClass>> {
    return this.request<CourseResponse[]>(
      `/instructors/${encodeURIComponent(slug)}/sections?term=${encodeURIComponent(term)}`
    );
  }

  // Admin endpoints
  async getAdminStatus(): Promise<Result<AdminStatusResponse, ApiErrorClass>> {
    return this.request<AdminStatusResponse>("/admin/status");
  }

  async getAdminUsers(): Promise<Result<User[], ApiErrorClass>> {
    return this.request<User[]>("/admin/users");
  }

  async setUserAdmin(discordId: string, isAdmin: boolean): Promise<Result<User, ApiErrorClass>> {
    return this.request<User>(`/admin/users/${discordId}/admin`, {
      method: "PUT",
      body: { is_admin: isAdmin },
    });
  }

  async getAdminScrapeJobs(): Promise<Result<ScrapeJobsResponse, ApiErrorClass>> {
    return this.request<ScrapeJobsResponse>("/admin/scrape-jobs");
  }

  /**
   * Fetch the audit log with conditional request support.
   *
   * Returns `ok(null)` when the server responds 304 (data unchanged).
   * Stores and sends `Last-Modified` / `If-Modified-Since` automatically.
   */
  async getAdminAuditLog(): Promise<Result<AuditLogResponse | null, ApiErrorClass>> {
    const headers: Record<string, string> = {};
    if (this._auditLastModified) {
      headers["If-Modified-Since"] = this._auditLastModified;
    }

    let response: Response;
    try {
      response = await this.fetchFn(`${this.baseUrl}/admin/audit-log`, { headers });
    } catch (e) {
      return err(
        new ApiErrorClass({
          code: "INTERNAL_ERROR",
          message: e instanceof Error ? e.message : "Network request failed",
          details: null,
        })
      );
    }

    if (response.status === 304) {
      return ok(null);
    }

    if (!response.ok) {
      let apiError: ApiError | undefined;
      try {
        apiError = (await response.json()) as ApiError;
      } catch {
        // Fall through — responseToErr uses a default
      }
      return this.responseToErr(response, apiError);
    }

    const lastMod = response.headers.get("Last-Modified");
    if (lastMod) {
      this._auditLastModified = lastMod;
    }

    return ok((await response.json()) as AuditLogResponse);
  }

  /** Stored `Last-Modified` value for audit log conditional requests. */
  private _auditLastModified: string | null = null;

  async getTimeline(ranges: TimeRange[]): Promise<Result<TimelineResponse, ApiErrorClass>> {
    return this.request<TimelineResponse>("/timeline", {
      method: "POST",
      body: { ranges } satisfies TimelineRequest,
    });
  }

  async getMetrics(
    params?: Partial<MetricsParams>
  ): Promise<Result<MetricsResponse, ApiErrorClass>> {
    if (!params) {
      return this.request<MetricsResponse>("/metrics");
    }
    const query = toURLSearchParams(params as Record<string, unknown>);
    const qs = query.toString();
    return this.request<MetricsResponse>(`/metrics${qs ? `?${qs}` : ""}`);
  }

  // Admin instructor endpoints

  async getAdminInstructors(
    params?: Partial<ListInstructorsParams>
  ): Promise<Result<ListInstructorsResponse, ApiErrorClass>> {
    if (!params) {
      return this.request<ListInstructorsResponse>("/admin/instructors");
    }
    const query = toURLSearchParams(params as Record<string, unknown>);
    const qs = query.toString();
    return this.request<ListInstructorsResponse>(`/admin/instructors${qs ? `?${qs}` : ""}`);
  }

  async getAdminInstructor(id: number): Promise<Result<InstructorDetailResponse, ApiErrorClass>> {
    return this.request<InstructorDetailResponse>(`/admin/instructors/${id}`);
  }

  async matchInstructor(
    id: number,
    rmpLegacyId: number
  ): Promise<Result<InstructorDetailResponse, ApiErrorClass>> {
    return this.request<InstructorDetailResponse>(`/admin/instructors/${id}/match`, {
      method: "POST",
      body: { rmpLegacyId } satisfies MatchBody,
    });
  }

  async rejectCandidate(id: number, rmpLegacyId: number): Promise<Result<void, ApiErrorClass>> {
    return this.requestVoid(`/admin/instructors/${id}/reject-candidate`, {
      method: "POST",
      body: { rmpLegacyId } satisfies RejectCandidateBody,
    });
  }

  async rejectAllCandidates(id: number): Promise<Result<void, ApiErrorClass>> {
    return this.requestVoid(`/admin/instructors/${id}/reject-all`, {
      method: "POST",
    });
  }

  async unmatchInstructor(id: number, rmpLegacyId?: number): Promise<Result<void, ApiErrorClass>> {
    return this.requestVoid(`/admin/instructors/${id}/unmatch`, {
      method: "POST",
      ...(rmpLegacyId !== undefined ? { body: { rmpLegacyId } satisfies MatchBody } : {}),
    });
  }

  async rescoreInstructors(): Promise<Result<RescoreResponse, ApiErrorClass>> {
    return this.request<RescoreResponse>("/admin/rmp/rescore", {
      method: "POST",
    });
  }

  // Scraper analytics endpoints

  async getScraperStats(
    period?: ScraperPeriod,
    term?: string
  ): Promise<Result<ScraperStatsResponse, ApiErrorClass>> {
    const query = new URLSearchParams();
    if (period) query.set("period", period);
    if (term) query.set("term", term);
    const qs = query.toString();
    return this.request<ScraperStatsResponse>(`/admin/scraper/stats${qs ? `?${qs}` : ""}`);
  }

  async getScraperTimeseries(
    period?: ScraperPeriod,
    bucket?: string,
    term?: string
  ): Promise<Result<TimeseriesResponse, ApiErrorClass>> {
    const query = new URLSearchParams();
    if (period) query.set("period", period);
    if (bucket) query.set("bucket", bucket);
    if (term) query.set("term", term);
    const qs = query.toString();
    return this.request<TimeseriesResponse>(`/admin/scraper/timeseries${qs ? `?${qs}` : ""}`);
  }

  async getScraperSubjects(): Promise<Result<SubjectsResponse, ApiErrorClass>> {
    return this.request<SubjectsResponse>("/admin/scraper/subjects");
  }

  async getScraperSubjectDetail(
    subject: string,
    limit?: number
  ): Promise<Result<SubjectDetailResponse, ApiErrorClass>> {
    const qs = limit !== undefined ? `?limit=${limit}` : "";
    return this.request<SubjectDetailResponse>(
      `/admin/scraper/subjects/${encodeURIComponent(subject)}${qs}`
    );
  }

  async getAdminTerms(): Promise<Result<TermsListResponse, ApiErrorClass>> {
    return this.request<TermsListResponse>("/admin/terms");
  }

  async enableTerm(code: string): Promise<Result<TermUpdateResponse, ApiErrorClass>> {
    return this.request<TermUpdateResponse>(`/admin/terms/${encodeURIComponent(code)}/enable`, {
      method: "POST",
    });
  }

  async disableTerm(code: string): Promise<Result<TermUpdateResponse, ApiErrorClass>> {
    return this.request<TermUpdateResponse>(`/admin/terms/${encodeURIComponent(code)}/disable`, {
      method: "POST",
    });
  }

  async syncTerms(): Promise<Result<TermSyncResponse, ApiErrorClass>> {
    return this.request<TermSyncResponse>("/admin/terms/sync", { method: "POST" });
  }

  async syncBlueBook(): Promise<Result<BluebookSyncTriggerResponse, ApiErrorClass>> {
    return this.request<BluebookSyncTriggerResponse>("/admin/bluebook/sync", { method: "POST" });
  }

  async getAdminBluebookLinks(
    params?: Partial<ListBluebookLinksParams>
  ): Promise<Result<ListBluebookLinksResponse, ApiErrorClass>> {
    if (!params) {
      return this.request<ListBluebookLinksResponse>("/admin/bluebook/links");
    }
    const query = toURLSearchParams(params as Record<string, unknown>);
    const qs = query.toString();
    return this.request<ListBluebookLinksResponse>(`/admin/bluebook/links${qs ? `?${qs}` : ""}`);
  }

  async getAdminBluebookLink(id: number): Promise<Result<BluebookLinkDetail, ApiErrorClass>> {
    return this.request<BluebookLinkDetail>(`/admin/bluebook/links/${id}`);
  }

  async approveBluebookLink(id: number): Promise<Result<BluebookOkResponse, ApiErrorClass>> {
    return this.request<BluebookOkResponse>(`/admin/bluebook/links/${id}/approve`, {
      method: "POST",
    });
  }

  async rejectBluebookLink(id: number): Promise<Result<BluebookOkResponse, ApiErrorClass>> {
    return this.request<BluebookOkResponse>(`/admin/bluebook/links/${id}/reject`, {
      method: "POST",
    });
  }

  async assignBluebookLink(
    id: number,
    instructorId: number
  ): Promise<Result<BluebookOkResponse, ApiErrorClass>> {
    return this.request<BluebookOkResponse>(`/admin/bluebook/links/${id}/assign`, {
      method: "POST",
      body: { instructorId } satisfies AssignBody,
    });
  }

  async runBluebookMatching(): Promise<Result<BluebookMatchResponse, ApiErrorClass>> {
    return this.request<BluebookMatchResponse>("/admin/bluebook/match", {
      method: "POST",
    });
  }

  async suggest(
    term: string,
    query: string,
    limit?: number
  ): Promise<Result<SuggestResponse, ApiErrorClass>> {
    const params = new URLSearchParams({ term, q: query });
    if (limit !== undefined) params.set("limit", String(limit));
    return this.request<SuggestResponse>(`/suggest?${params.toString()}`);
  }
}

export const client = new BannerApiClient();

export type { Result } from "true-myth/result";

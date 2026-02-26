/** Page view (PostHog standard — captured via afterNavigate, not history monkey-patching) */
export interface PageViewEvent {
  name: "$pageview";
  properties: {
    route: string;
    referrer?: string;
  };
}

/** Course search and filter interactions */
export interface CourseSearchEvent {
  name: "course_search";
  properties: {
    action: "query" | "filter" | "sort" | "clear";
    query?: string;
    filterType?: string;
    filterValue?: string;
    sortBy?: string;
    term?: string;
    resultCount?: number;
  };
}

/** Course and section detail interactions */
export interface CourseInteractionEvent {
  name: "course_interaction";
  properties: {
    action: "detail_view" | "section_expand" | "instructor_click";
    crn?: string;
    term?: string;
    courseCode?: string;
  };
}

/** User auth events (logout only — login is handled implicitly via identify) */
export interface AuthEvent {
  name: "auth";
  properties: {
    action: "logout";
  };
}

/** Client/server error tracking */
export interface ErrorEvent {
  name: "error";
  properties: {
    errorType: "network_error" | "validation_error" | "runtime_error" | (string & {});
    message: string;
    stack?: string;
    context?: Record<string, unknown>;
  };
}

/** Discriminated union of all telemetry events */
export type TelemetryEvent =
  | PageViewEvent
  | CourseSearchEvent
  | CourseInteractionEvent
  | AuthEvent
  | ErrorEvent;

/** Extract the properties type for a given event name */
export type EventProperties<T extends TelemetryEvent["name"]> = Extract<
  TelemetryEvent,
  { name: T }
>["properties"];

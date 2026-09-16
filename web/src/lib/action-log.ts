import type { ActionLogParams, AdminAction, AdminEntity } from "$lib/bindings";

/**
 * Reviewer-facing name for every recorded action. Keyed by the union, so a new
 * backend variant fails the type check until it is named here.
 */
export const ACTION_LABELS: Record<AdminAction, string> = {
  instructor_merge: "Merge instructors",
  instructor_merge_claimant: "Merge into RMP claimant",
  instructor_merge_all: "Merge all duplicates",
  instructor_dismiss: "Dismiss duplicate pair",
  instructor_undismiss: "Restore dismissed pair",
  rmp_accept_candidate: "Accept RMP candidate",
  rmp_reject_candidate: "Reject RMP candidate",
  rmp_reject_all: "Reject all RMP candidates",
  rmp_unmatch: "Unmatch RMP profile",
  bluebook_approve_link: "Approve BlueBook link",
  bluebook_reject_link: "Reject BlueBook link",
  bluebook_assign_link: "Assign BlueBook link",
  user_set_admin: "Change admin access",
  term_enable: "Enable term scraping",
  term_disable: "Disable term scraping",
};

export const ENTITY_LABELS: Record<AdminEntity, string> = {
  instructor: "Instructor",
  user: "User",
  term: "Term",
  bluebook_link: "BlueBook link",
};

export const ACTIONS = Object.keys(ACTION_LABELS) as AdminAction[];
export const ENTITIES = Object.keys(ENTITY_LABELS) as AdminEntity[];

export const ACTION_LOG_PER_PAGE = 50;

function trimmed(value: string | null): string | null {
  const text = value?.trim() ?? "";
  return text.length > 0 ? text : null;
}

/** Read filters out of the URL, dropping anything the backend would reject. */
export function parseActionLogParams(search: URLSearchParams): ActionLogParams {
  const action = trimmed(search.get("action"));
  const entityType = trimmed(search.get("entityType"));
  const page = Number.parseInt(search.get("page") ?? "", 10);

  return {
    actor: trimmed(search.get("actor")),
    action: action && action in ACTION_LABELS ? (action as AdminAction) : null,
    entityType: entityType && entityType in ENTITY_LABELS ? (entityType as AdminEntity) : null,
    entityId: trimmed(search.get("entityId")),
    since: trimmed(search.get("since")),
    until: trimmed(search.get("until")),
    page: Number.isNaN(page) || page < 1 ? 1 : page,
    perPage: ACTION_LOG_PER_PAGE,
  };
}

/** Write filters back to the URL, leaving defaults out so links stay readable. */
export function serializeActionLogParams(params: ActionLogParams): URLSearchParams {
  const search = new URLSearchParams();
  if (params.actor) search.set("actor", params.actor);
  if (params.action) search.set("action", params.action);
  if (params.entityType) search.set("entityType", params.entityType);
  if (params.entityId) search.set("entityId", params.entityId);
  if (params.since) search.set("since", params.since);
  if (params.until) search.set("until", params.until);
  if (params.page && params.page > 1) search.set("page", String(params.page));
  return search;
}

export function countActiveFilters(params: ActionLogParams): number {
  return [
    params.actor,
    params.action,
    params.entityType,
    params.entityId,
    params.since,
    params.until,
  ].filter(Boolean).length;
}

/** `datetime-local` speaks local wall time; the API speaks UTC instants. */
export function toLocalInput(iso: string | null | undefined): string {
  if (!iso) return "";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(date.getTime() - offset).toISOString().slice(0, 16);
}

export function fromLocalInput(value: string): string | null {
  if (!value) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date.toISOString();
}

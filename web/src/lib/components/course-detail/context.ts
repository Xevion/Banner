import type { CodeDescription } from "$lib/bindings";
import { getContext, setContext } from "svelte";

const COURSE_DETAIL_CONTEXT_KEY = Symbol("course-detail-context");

export interface CourseDetailContext {
  /** Attribute code -> human-readable description. Reactive via getter. */
  readonly attributeMap: Record<string, string>;
  /** Navigate to a different section's CRN in the course table. */
  navigateToSection: ((crn: string) => void) | null;
}

/** Set the course detail context (call during component init). */
export function setCourseDetailContext(ctx: CourseDetailContext): void {
  setContext(COURSE_DETAIL_CONTEXT_KEY, ctx);
}

/** Get the course detail context (call from a descendant component). */
export function getCourseDetailContext(): CourseDetailContext | undefined {
  return getContext<CourseDetailContext | undefined>(COURSE_DETAIL_CONTEXT_KEY);
}

/** Build an attribute map from a CodeDescription array. */
export function buildAttributeMap(attributes: CodeDescription[]): Record<string, string> {
  return Object.fromEntries(attributes.map((a) => [a.code, a.description]));
}

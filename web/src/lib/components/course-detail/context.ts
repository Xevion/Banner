import type { CodeDescription } from "$lib/bindings";
import { createContext } from "svelte";

export interface CourseDetailContext {
  /** Attribute code -> human-readable description. Reactive via getter. */
  readonly attributeMap: Record<string, string>;
  /** Navigate to a different section's CRN in the course table. */
  navigateToSection: ((crn: string) => void) | null;
}

export const [getCourseDetailContext, setCourseDetailContext] =
  createContext<CourseDetailContext>();

/** Build an attribute map from a CodeDescription array. */
export function buildAttributeMap(attributes: CodeDescription[]): Record<string, string> {
  return Object.fromEntries(attributes.map((a) => [a.code, a.description]));
}

import { createContext } from "svelte";

export interface CourseDetailContext {
  /** Navigate to a different section's CRN in the course table. */
  navigateToSection: ((crn: string) => void) | null;
}

export const [getCourseDetailContext, setCourseDetailContext] =
  createContext<CourseDetailContext>();

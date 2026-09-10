import microfuzz from "@nozbe/microfuzz";
import type { Subject } from "$lib/api";
import type { CourseSuggestion, InstructorSuggestion, SuggestResponse } from "$lib/bindings";

/** Shorter than this and every subject in the catalogue matches. */
export const MIN_QUERY_LENGTH = 2;

export type Suggestion =
  | { kind: "subject"; subject: Subject }
  | { kind: "course"; course: CourseSuggestion }
  | { kind: "instructor"; instructor: InstructorSuggestion };

export type SubjectSearch = (query: string) => { item: Subject; score: number }[];

// microfuzz is CJS with an `__esModule` default, and rolldown hands the default
// import the whole exports object instead of unwrapping it.
const createFuzzySearch =
  (microfuzz as unknown as { default?: typeof microfuzz }).default ?? microfuzz;

export function createSubjectSearch(subjects: Subject[]): SubjectSearch {
  return createFuzzySearch(subjects, {
    getText: (item: Subject) => [item.code, item.description],
  });
}

/** Stable across renders and unique even when a title contains the separator. */
export function suggestionId(suggestion: Suggestion): string {
  switch (suggestion.kind) {
    case "subject":
      return `subject:${suggestion.subject.code}`;
    case "course":
      return `course:${suggestion.course.subject}:${suggestion.course.courseNumber}:${suggestion.course.title}`;
    case "instructor":
      return `instructor:${suggestion.instructor.id}`;
  }
}

const MAX_SUBJECTS = 5;

/**
 * Rescales a microfuzz score so it can be ranked against a server one.
 *
 * Server suggestions carry a trigram similarity, 0 to 1 and higher is better,
 * while microfuzz reports an error level starting at 0 for an exact hit and
 * growing without bound. Only the ordering carries over; microfuzz documents its
 * exact values as free to change between releases.
 */
function fuzzyToSimilarity(errorLevel: number): number {
  return 1 / (1 + errorLevel);
}

export function mergeSuggestions(
  query: string,
  searchSubjects: SubjectSearch,
  server: SuggestResponse,
  excludedInstructors: ReadonlySet<string>
): Suggestion[] {
  const q = query.trim();
  if (q.length < MIN_QUERY_LENGTH) return [];

  const scored: { suggestion: Suggestion; score: number }[] = [];

  for (const hit of searchSubjects(q).slice(0, MAX_SUBJECTS)) {
    scored.push({
      suggestion: { kind: "subject", subject: hit.item },
      score: fuzzyToSimilarity(hit.score),
    });
  }
  for (const c of server.courses) {
    scored.push({ suggestion: { kind: "course", course: c }, score: c.score });
  }
  for (const i of server.instructors) {
    if (!excludedInstructors.has(i.slug)) {
      scored.push({ suggestion: { kind: "instructor", instructor: i }, score: i.score });
    }
  }

  scored.sort((a, b) => b.score - a.score);
  return scored.map((s) => s.suggestion);
}

import { describe, expect, it } from "vitest";
import type { Subject } from "$lib/api";
import type { CourseSuggestion, InstructorSuggestion, SuggestResponse } from "$lib/bindings";
import { createSubjectSearch, mergeSuggestions, suggestionId } from "./suggestions";

const subjects: Subject[] = [
  { code: "CS", description: "Computer Science", filterValue: "CS" },
  { code: "MAT", description: "Mathematics", filterValue: "MAT" },
  { code: "ECE", description: "Electrical and Computer Engineering", filterValue: "ECE" },
];

const search = createSubjectSearch(subjects);

function course(title: string, score: number): CourseSuggestion {
  return { subject: "CS", courseNumber: "3443", title, sectionCount: 2, score };
}

function instructor(slug: string, score: number): InstructorSuggestion {
  return { id: 1, slug, displayName: slug, sectionCount: 1, score };
}

const noServer: SuggestResponse = { courses: [], instructors: [] };
const none = new Set<string>();

/** The kind/label pairs a caller would render, in the order they come back. */
function labels(items: ReturnType<typeof mergeSuggestions>): string[] {
  return items.map((i) =>
    i.kind === "subject"
      ? `subject:${i.subject.code}`
      : i.kind === "course"
        ? `course:${i.course.title}`
        : `instructor:${i.instructor.slug}`
  );
}

describe("query threshold", () => {
  it("suggests nothing until the query is long enough to mean something", () => {
    expect(mergeSuggestions("c", search, noServer, none)).toEqual([]);
    expect(mergeSuggestions(" ", search, noServer, none)).toEqual([]);
  });

  it("ignores surrounding whitespace when measuring the query", () => {
    expect(mergeSuggestions("  cs  ", search, noServer, none)).not.toEqual([]);
  });
});

describe("ranking", () => {
  it("puts an exactly matching subject code above a middling course match", () => {
    const server: SuggestResponse = {
      courses: [course("Introduction to CS", 0.6)],
      instructors: [],
    };
    expect(labels(mergeSuggestions("cs", search, server, none))[0]).toBe("subject:CS");
  });

  it("puts a strong course match above a subject that only fuzzily matches", () => {
    const server: SuggestResponse = {
      courses: [course("Computer Networks", 0.95)],
      instructors: [],
    };
    expect(labels(mergeSuggestions("comput", search, server, none))[0]).toBe(
      "course:Computer Networks"
    );
  });

  it("orders subjects among themselves by how well they match", () => {
    // "Computer Science" matches the code exactly; ECE only contains the word.
    const ordered = labels(mergeSuggestions("computer", search, noServer, none));
    expect(ordered.indexOf("subject:CS")).toBeLessThan(ordered.indexOf("subject:ECE"));
  });

  it("interleaves the three kinds by match quality rather than grouping them", () => {
    const server: SuggestResponse = {
      courses: [course("Computer Networks", 0.9)],
      instructors: [instructor("ada-lovelace", 0.3)],
    };
    const ordered = labels(mergeSuggestions("comput", search, server, none));
    expect(ordered.at(-1)).toBe("instructor:ada-lovelace");
    expect(ordered).toContain("subject:CS");
  });
});

describe("exclusions", () => {
  it("drops instructors that are already applied as a filter", () => {
    const server: SuggestResponse = {
      courses: [],
      instructors: [instructor("ada-lovelace", 0.9), instructor("alan-turing", 0.8)],
    };
    const ordered = labels(mergeSuggestions("turing", search, server, new Set(["ada-lovelace"])));
    expect(ordered).not.toContain("instructor:ada-lovelace");
    expect(ordered).toContain("instructor:alan-turing");
  });
});

describe("identity", () => {
  it("gives distinct suggestions distinct ids", () => {
    const server: SuggestResponse = {
      courses: [course("Data Structures: Advanced", 0.9), course("Data Structures", 0.9)],
      instructors: [],
    };
    const ids = mergeSuggestions("data", search, server, none).map(suggestionId);
    expect(new Set(ids).size).toBe(ids.length);
  });
});

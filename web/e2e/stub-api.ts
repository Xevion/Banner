/**
 * Canned `/api` responses for the end-to-end smoke tests.
 *
 * The built app proxies `/api` to `BACKEND_URL` inside `hooks.server.ts`, so a
 * single stub answers both the SSR-time and the browser-time requests.
 */
import { createServer } from "node:http";
import type { ServerResponse } from "node:http";
import { mockCourses } from "../src/lib/stories/fixtures/courses";

const port = Number(process.env.E2E_STUB_PORT ?? 8788);

/**
 * Subjects the autocomplete fuzzy-matches client side. "Computer Science" is
 * the only entry reachable from the query the smoke test types.
 */
const subjects = [
  { code: "CS", description: "Computer Science", filterValue: "CS" },
  { code: "MAT", description: "Mathematics", filterValue: "MAT" },
  { code: "ENG", description: "English", filterValue: "ENG" },
  { code: "PHY", description: "Physics", filterValue: "PHY" },
  { code: "CHE", description: "Chemistry", filterValue: "CHE" },
  { code: "HIS", description: "History", filterValue: "HIS" },
];

const searchOptions = {
  terms: [
    { code: "202620", slug: "fall-2026", description: "Fall 2026" },
    { code: "202710", slug: "spring-2027", description: "Spring 2027" },
  ],
  subjects,
  reference: {
    instructionalMethods: [
      { code: "P", description: "In Person", filterValue: "InPerson" },
      { code: "INET", description: "Online Asynchronous", filterValue: "Online.Async" },
    ],
    campuses: [{ code: "1", description: "Main", filterValue: "Main" }],
    partsOfTerm: [{ code: "1", description: "Full Term", filterValue: "FullTerm" }],
    attributes: [{ code: "CORE", description: "Core Curriculum", filterValue: "raw:CORE" }],
  },
  ranges: {
    courseNumberMin: 1000,
    courseNumberMax: 7000,
    creditHourMin: 0,
    creditHourMax: 6,
    waitCountMax: 25,
  },
  sorts: [
    { key: "course_code", ascLabel: "A to Z", descLabel: "Z to A" },
    { key: "title", ascLabel: "A to Z", descLabel: "Z to A" },
    { key: "instructor_name", ascLabel: "A to Z", descLabel: "Z to A" },
    { key: "instructor_rating", ascLabel: "Lowest", descLabel: "Highest" },
    { key: "start_time", ascLabel: "Earliest", descLabel: "Latest" },
    { key: "end_time", ascLabel: "Earliest", descLabel: "Latest" },
    { key: "duration", ascLabel: "Shortest", descLabel: "Longest" },
    { key: "days", ascLabel: "Fewest", descLabel: "Most" },
    { key: "seats_open", ascLabel: "Fewest", descLabel: "Most" },
    { key: "fill_ratio", ascLabel: "Emptiest", descLabel: "Fullest" },
    { key: "wait_count", ascLabel: "Fewest", descLabel: "Most" },
    { key: "weekly_minutes", ascLabel: "Least", descLabel: "Most" },
  ],
};

/** Server-side half of the autocomplete, returned for any query. */
const suggestions = {
  courses: [
    {
      subject: "CS",
      courseNumber: "3443",
      title: "Application Programming",
      sectionCount: 4,
      score: 0.8,
    },
  ],
  instructors: [
    { id: 1001, slug: "john-smith-abc", displayName: "John Smith", sectionCount: 3, score: 0.6 },
  ],
};

function json(res: ServerResponse, body: unknown, status = 200): void {
  const payload = JSON.stringify(body);
  res.writeHead(status, {
    "content-type": "application/json",
    "content-length": Buffer.byteLength(payload),
  });
  res.end(payload);
}

const server = createServer((req, res) => {
  const path = new URL(req.url ?? "/", `http://localhost:${port}`).pathname;

  switch (path) {
    case "/api/health":
      return json(res, { status: "healthy", timestamp: new Date().toISOString() });
    case "/api/auth/me":
      return json(res, null);
    case "/api/search-options":
      return json(res, searchOptions);
    case "/api/courses/search":
      return json(res, { courses: mockCourses, totalCount: mockCourses.length });
    case "/api/suggest":
      return json(res, suggestions);
    case "/api/instructors/resolve":
      return json(res, {});
    case "/api/csp-report":
      res.writeHead(204);
      return res.end();
    default:
      // Loud on purpose: an unstubbed endpoint should be obvious in the logs.
      process.stderr.write(`stub-api: no handler for ${path}\n`);
      return json(res, { code: "NOT_FOUND", message: `No stub for ${path}`, details: null }, 404);
  }
});

server.listen(port, "127.0.0.1", () => {
  process.stdout.write(`stub-api listening on http://127.0.0.1:${port}\n`);
});

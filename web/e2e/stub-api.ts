/**
 * Canned `/api` responses for the end-to-end smoke tests.
 *
 * The built app proxies `/api` to `BACKEND_URL` inside `hooks.server.ts`, so a
 * single stub answers both the SSR-time and the browser-time requests.
 */
import { createServer } from "node:http";
import type { IncomingMessage, ServerResponse } from "node:http";
import type {
  CourseResponse,
  TrendSample,
  TrendsRequest,
  TrendsResponse,
} from "../src/lib/bindings";
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

/** History walked back from a section's current numbers, so its sparkline draws a path. */
function trendFor(course: CourseResponse): TrendSample[] {
  const { current, max, waitCount } = course.enrollment;
  return Array.from({ length: 6 }, (_, index) => {
    const stepsBack = 5 - index;
    const enrollment = Math.max(0, current - stepsBack * 2);
    return {
      enrollment,
      waitCount: Math.max(0, waitCount - stepsBack),
      seatsAvailable: Math.max(0, max - enrollment),
    };
  });
}

const trendsByCrn = new Map(mockCourses.map((course) => [course.crn, trendFor(course)]));

/** Like the real endpoint, a section with no recorded history is absent. */
function courseTrends(body: TrendsRequest): TrendsResponse {
  const trends: TrendsResponse["trends"] = {};
  for (const crn of body.crns) {
    const samples = trendsByCrn.get(crn);
    if (samples) trends[crn] = samples;
  }
  return { trends };
}

async function readJsonBody(req: IncomingMessage): Promise<unknown> {
  req.setEncoding("utf8");
  let raw = "";
  for await (const chunk of req) raw += chunk as string;
  return raw ? JSON.parse(raw) : {};
}

/**
 * Names for the slugs asked about, drawn from the same instructors the
 * autocomplete offers.
 *
 * Answering `{}` would make every chip fall back to its slug, which is what a
 * page looks like when name resolution is broken.
 */
function resolveInstructors(url: URL): Record<string, string> {
  const known = new Map(suggestions.instructors.map((i) => [i.slug, i.displayName]));
  const resolved: Record<string, string> = {};
  for (const slug of url.searchParams.getAll("slug")) {
    const name = known.get(slug);
    if (name) resolved[slug] = name;
  }
  return resolved;
}

async function handle(req: IncomingMessage, res: ServerResponse): Promise<void> {
  const url = new URL(req.url ?? "/", `http://localhost:${port}`);
  const path = url.pathname;

  switch (path) {
    case "/api/courses/trends":
      json(res, courseTrends((await readJsonBody(req)) as TrendsRequest));
      break;
    case "/api/health":
      json(res, { status: "healthy", timestamp: new Date().toISOString() });
      break;
    case "/api/auth/me":
      json(res, null);
      break;
    case "/api/search-options":
      json(res, searchOptions);
      break;
    case "/api/courses/search":
      json(res, { courses: mockCourses, totalCount: mockCourses.length });
      break;
    case "/api/suggest":
      json(res, suggestions);
      break;
    case "/api/instructors/suggest":
      json(res, suggestions.instructors);
      break;
    case "/api/instructors/resolve":
      json(res, resolveInstructors(url));
      break;
    case "/api/timeline":
      json(res, { slots: [], subjects: [] });
      break;
    case "/api/csp-report":
      res.writeHead(204);
      res.end();
      break;
    default:
      // Loud on purpose: an unstubbed endpoint should be obvious in the logs.
      process.stderr.write(`stub-api: no handler for ${path}\n`);
      json(res, { code: "NOT_FOUND", message: `No stub for ${path}`, details: null }, 404);
  }
}

const server = createServer((req, res) => {
  void handle(req, res);
});

server.listen(port, "127.0.0.1", () => {
  process.stdout.write(`stub-api listening on http://127.0.0.1:${port}\n`);
});

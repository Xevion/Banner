import { describe, it, expect } from "vitest";
import { formatMeetingTimeSummary } from "$lib/course";
import type { CourseResponse, DbMeetingTime } from "$lib/bindings";

function makeMeetingTime(overrides: Partial<DbMeetingTime> = {}): DbMeetingTime {
  const mt: DbMeetingTime = {
    timeRange: null,
    dateRange: { start: "2025-01-13", end: "2025-05-08" },
    days: [],
    location: null,
    meetingType: "CLAS",
    meetingScheduleType: "LEC",
    ...overrides,
  };
  return mt;
}

function makeCourse(overrides: Partial<CourseResponse> = {}): CourseResponse {
  return {
    crn: "12345",
    subject: "CS",
    courseNumber: "1234",
    title: "Test Course",
    termCode: "202510",
    sequenceNumber: null,
    instructionalMethod: null,
    instructionalMethodCode: null,
    campus: null,
    enrollment: { current: 10, max: 30, waitCount: 0, waitCapacity: 0 },
    creditHours: { type: "fixed", hours: 3 },
    crossList: null,
    sectionLink: null,
    partOfTerm: null,
    isAsyncOnline: false,
    primaryLocation: null,
    hasPhysicalLocation: false,
    primaryInstructorId: null,
    meetingTimes: [],
    attributes: [],
    instructors: [],
    ...overrides,
  };
}

describe("formatMeetingTimeSummary", () => {
  it("returns 'Async' for async online courses", () => {
    const course = makeCourse({
      isAsyncOnline: true,
      meetingTimes: [
        makeMeetingTime({
          location: { building: "INT", buildingDescription: null, room: null, campus: null },
        }),
      ],
    });
    expect(formatMeetingTimeSummary(course)).toBe("Async");
  });

  it("returns 'TBA' for courses with no meeting times", () => {
    const course = makeCourse({ meetingTimes: [] });
    expect(formatMeetingTimeSummary(course)).toBe("TBA");
  });

  it("returns 'TBA' when days and times are all TBA", () => {
    const course = makeCourse({
      meetingTimes: [makeMeetingTime()],
    });
    expect(formatMeetingTimeSummary(course)).toBe("TBA");
  });

  it("returns formatted days and time for normal meeting", () => {
    const course = makeCourse({
      meetingTimes: [
        makeMeetingTime({
          days: ["monday", "wednesday", "friday"],
          timeRange: { start: "09:00:00", end: "09:50:00" },
        }),
      ],
    });
    expect(formatMeetingTimeSummary(course)).toBe("MWF 9:00–9:50 AM");
  });

  it("returns formatted days with TBA time", () => {
    const course = makeCourse({
      meetingTimes: [
        makeMeetingTime({
          days: ["tuesday", "thursday"],
        }),
      ],
    });
    // Days are set but time is TBA — not both TBA, so it enters the final branch
    expect(formatMeetingTimeSummary(course)).toBe("TTh TBA");
  });
});

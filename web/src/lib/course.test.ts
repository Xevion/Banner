import type { CourseResponse, DbMeetingTime, InstructorResponse } from "$lib/bindings";
import {
  abbreviateInstructor,
  formatCreditHours,
  formatISOTime,
  formatInstructorName,
  formatMeetingDays,
  formatMeetingDaysLong,
  formatMeetingDaysVerbose,
  formatMeetingTimeSummary,
  formatMeetingTimeTooltip,
  formatMeetingTimesTooltip,
  formatTimeRange,
  getPrimaryInstructor,
} from "$lib/course";
import { formatDate, formatDateShort } from "$lib/date";
import { describe, expect, it } from "vitest";

function makeMeetingTime(overrides: Partial<DbMeetingTime> = {}): DbMeetingTime {
  const mt: DbMeetingTime = {
    timeRange: null,
    dateRange: { start: "2024-08-26", end: "2024-12-12" },
    days: [],
    location: null,
    meetingType: "CLAS",
    meetingScheduleType: "LEC",
    ...overrides,
  };
  return mt;
}

describe("formatISOTime", () => {
  it("converts 09:00:00 to 9:00 AM", () => expect(formatISOTime("09:00:00")).toBe("9:00 AM"));
  it("converts 13:30:00 to 1:30 PM", () => expect(formatISOTime("13:30:00")).toBe("1:30 PM"));
  it("converts 00:00:00 to 12:00 AM", () => expect(formatISOTime("00:00:00")).toBe("12:00 AM"));
  it("converts 12:00:00 to 12:00 PM", () => expect(formatISOTime("12:00:00")).toBe("12:00 PM"));
  it("converts 23:59:00 to 11:59 PM", () => expect(formatISOTime("23:59:00")).toBe("11:59 PM"));
  it("returns TBA for null", () => expect(formatISOTime(null)).toBe("TBA"));
});

describe("formatMeetingDays", () => {
  it("returns MWF for mon/wed/fri", () => {
    expect(formatMeetingDays(makeMeetingTime({ days: ["monday", "wednesday", "friday"] }))).toBe(
      "MWF"
    );
  });
  it("returns TTh for tue/thu", () => {
    expect(formatMeetingDays(makeMeetingTime({ days: ["tuesday", "thursday"] }))).toBe("TTh");
  });
  it("returns MW for mon/wed", () => {
    expect(formatMeetingDays(makeMeetingTime({ days: ["monday", "wednesday"] }))).toBe("MW");
  });
  it("returns MTWThF for all weekdays", () => {
    expect(
      formatMeetingDays(
        makeMeetingTime({ days: ["monday", "tuesday", "wednesday", "thursday", "friday"] })
      )
    ).toBe("MTWThF");
  });
  it("returns partial abbreviation for single day", () => {
    expect(formatMeetingDays(makeMeetingTime({ days: ["monday"] }))).toBe("Mon");
    expect(formatMeetingDays(makeMeetingTime({ days: ["thursday"] }))).toBe("Thu");
    expect(formatMeetingDays(makeMeetingTime({ days: ["saturday"] }))).toBe("Sat");
  });
  it("concatenates codes for other multi-day combos", () => {
    expect(formatMeetingDays(makeMeetingTime({ days: ["monday", "friday"] }))).toBe("MF");
    expect(formatMeetingDays(makeMeetingTime({ days: ["tuesday", "saturday"] }))).toBe("TSa");
    expect(formatMeetingDays(makeMeetingTime({ days: ["wednesday", "friday", "sunday"] }))).toBe(
      "WFSu"
    );
    expect(
      formatMeetingDays(makeMeetingTime({ days: ["monday", "tuesday", "wednesday", "thursday"] }))
    ).toBe("MTWTh");
  });
  it("returns empty string when no days", () => {
    expect(formatMeetingDays(makeMeetingTime())).toBe("");
  });
});

describe("formatTimeRange", () => {
  it("elides AM when both times are AM", () => {
    expect(formatTimeRange("09:00:00", "09:50:00")).toBe("9:00–9:50 AM");
  });
  it("elides PM when both times are PM", () => {
    expect(formatTimeRange("13:15:00", "14:30:00")).toBe("1:15–2:30 PM");
  });
  it("keeps both markers when crossing noon", () => {
    expect(formatTimeRange("11:30:00", "12:20:00")).toBe("11:30 AM–12:20 PM");
  });
  it("returns TBA for null begin", () => {
    expect(formatTimeRange(null, "09:50:00")).toBe("TBA");
  });
  it("returns TBA for null end", () => {
    expect(formatTimeRange("09:00:00", null)).toBe("TBA");
  });
  it("handles midnight and noon", () => {
    expect(formatTimeRange("00:00:00", "00:50:00")).toBe("12:00–12:50 AM");
    expect(formatTimeRange("12:00:00", "12:50:00")).toBe("12:00–12:50 PM");
  });
});

describe("abbreviateInstructor", () => {
  it("returns short names unabbreviated", () =>
    expect(abbreviateInstructor("Li, Bo")).toBe("Li, Bo"));
  it("returns names within budget unabbreviated", () =>
    expect(abbreviateInstructor("Heaps, John")).toBe("Heaps, John"));
  it("handles no comma", () => expect(abbreviateInstructor("Staff")).toBe("Staff"));

  // Progressive abbreviation with multiple given names
  it("abbreviates trailing given names first", () =>
    expect(abbreviateInstructor("Ramirez, Maria Elena")).toBe("Ramirez, Maria E."));
  it("abbreviates all given names when needed", () =>
    expect(abbreviateInstructor("Ramirez, Maria Elena", 16)).toBe("Ramirez, M. E."));
  it("falls back to first initial only", () =>
    expect(abbreviateInstructor("Ramirez, Maria Elena", 12)).toBe("Ramirez, M."));

  // Single given name that exceeds budget
  it("abbreviates single given name when over budget", () =>
    expect(abbreviateInstructor("Bartholomew, Christopher", 18)).toBe("Bartholomew, C."));

  // Respects custom maxLen
  it("keeps full name when within custom budget", () =>
    expect(abbreviateInstructor("Ramirez, Maria Elena", 30)).toBe("Ramirez, Maria Elena"));
  it("always abbreviates when budget is tiny", () =>
    expect(abbreviateInstructor("Heaps, John", 5)).toBe("Heaps, J."));
});

describe("getPrimaryInstructor", () => {
  it("returns primary instructor", () => {
    const instructors: InstructorResponse[] = [
      {
        instructorId: 1,
        bannerId: "1",
        displayName: "A",
        firstName: null,
        lastName: null,
        email: "a@utsa.edu",
        isPrimary: false,
        rmp: null,
        bluebook: null,
        composite: null,
        slug: null,
      },
      {
        instructorId: 2,
        bannerId: "2",
        displayName: "B",
        firstName: null,
        lastName: null,
        email: "b@utsa.edu",
        isPrimary: true,
        rmp: null,
        bluebook: null,
        composite: null,
        slug: null,
      },
    ];
    expect(getPrimaryInstructor(instructors)?.displayName).toBe("B");
  });
  it("returns first instructor when no primary", () => {
    const instructors: InstructorResponse[] = [
      {
        instructorId: 3,
        bannerId: "1",
        displayName: "A",
        firstName: null,
        lastName: null,
        email: "a@utsa.edu",
        isPrimary: false,
        rmp: null,
        bluebook: null,
        composite: null,
        slug: null,
      },
    ];
    expect(getPrimaryInstructor(instructors)?.displayName).toBe("A");
  });
  it("returns undefined for empty array", () => {
    expect(getPrimaryInstructor([])).toBeUndefined();
  });
});

describe("formatCreditHours", () => {
  it("returns creditHours when set", () => {
    expect(
      formatCreditHours({
        creditHours: { type: "fixed", hours: 3 },
      } as CourseResponse)
    ).toBe("3");
  });
  it("returns range when variable", () => {
    expect(
      formatCreditHours({
        creditHours: { type: "range", low: 1, high: 3 },
      } as CourseResponse)
    ).toBe("1–3");
  });
  it("returns dash when no credit info", () => {
    expect(
      formatCreditHours({
        creditHours: null,
      } as CourseResponse)
    ).toBe("—");
  });
});

describe("formatDate", () => {
  it("formats standard date", () => {
    expect(formatDate("2024-08-26")).toBe("August 26, 2024");
  });
  it("formats December date", () => {
    expect(formatDate("2024-12-12")).toBe("December 12, 2024");
  });
  it("formats January 1st", () => {
    expect(formatDate("2026-01-01")).toBe("January 1, 2026");
  });
  it("returns original string for invalid input", () => {
    expect(formatDate("bad-date")).toBe("bad-date");
  });
});

describe("formatMeetingDaysLong", () => {
  it("returns full plural for single day", () => {
    expect(formatMeetingDaysLong(makeMeetingTime({ days: ["thursday"] }))).toBe("Thursdays");
  });
  it("returns full plural for Monday only", () => {
    expect(formatMeetingDaysLong(makeMeetingTime({ days: ["monday"] }))).toBe("Mondays");
  });
  it("returns semi-abbreviated for multiple days", () => {
    expect(
      formatMeetingDaysLong(makeMeetingTime({ days: ["monday", "wednesday", "friday"] }))
    ).toBe("Mon, Wed, Fri");
  });
  it("returns semi-abbreviated for TR", () => {
    expect(formatMeetingDaysLong(makeMeetingTime({ days: ["tuesday", "thursday"] }))).toBe(
      "Tue, Thu"
    );
  });
  it("returns empty string when no days", () => {
    expect(formatMeetingDaysLong(makeMeetingTime())).toBe("");
  });
});

describe("formatDateShort", () => {
  it("formats YYYY-MM-DD to short", () => {
    expect(formatDateShort("2024-08-26")).toBe("Aug 26, 2024");
  });
  it("formats December date to short", () => {
    expect(formatDateShort("2024-12-12")).toBe("Dec 12, 2024");
  });
  it("returns original for invalid", () => {
    expect(formatDateShort("bad")).toBe("bad");
  });
});

describe("formatMeetingDaysVerbose", () => {
  it("returns plural for single day", () => {
    expect(formatMeetingDaysVerbose(makeMeetingTime({ days: ["thursday"] }))).toBe("Thursdays");
  });
  it("joins two days with ampersand", () => {
    expect(formatMeetingDaysVerbose(makeMeetingTime({ days: ["tuesday", "thursday"] }))).toBe(
      "Tuesdays & Thursdays"
    );
  });
  it("uses Oxford-style ampersand for 3+ days", () => {
    expect(
      formatMeetingDaysVerbose(makeMeetingTime({ days: ["monday", "wednesday", "friday"] }))
    ).toBe("Mondays, Wednesdays & Fridays");
  });
  it("returns empty string when no days", () => {
    expect(formatMeetingDaysVerbose(makeMeetingTime())).toBe("");
  });
});

describe("formatMeetingTimeTooltip", () => {
  it("formats full tooltip with location and dates", () => {
    const mt = makeMeetingTime({
      days: ["tuesday", "thursday"],
      timeRange: { start: "16:15:00", end: "17:30:00" },
      location: { building: null, buildingDescription: "Main Hall", room: "2.206", campus: null },
    });
    expect(formatMeetingTimeTooltip(mt)).toBe(
      "Tuesdays & Thursdays, 4:15–5:30 PM\nMain Hall 2.206, Aug 26, 2024 – Dec 12, 2024"
    );
  });
  it("handles TBA days and times", () => {
    expect(formatMeetingTimeTooltip(makeMeetingTime())).toBe("TBA\nAug 26, 2024 – Dec 12, 2024");
  });
  it("handles days with TBA times", () => {
    expect(formatMeetingTimeTooltip(makeMeetingTime({ days: ["monday"] }))).toBe(
      "Mondays, TBA\nAug 26, 2024 – Dec 12, 2024"
    );
  });
});

describe("formatMeetingTimesTooltip", () => {
  it("returns TBA for empty array", () => {
    expect(formatMeetingTimesTooltip([])).toBe("TBA");
  });
  it("joins multiple meetings with blank line", () => {
    const mts = [
      makeMeetingTime({
        days: ["monday", "wednesday", "friday"],
        timeRange: { start: "09:00:00", end: "09:50:00" },
      }),
      makeMeetingTime({
        days: ["thursday"],
        timeRange: { start: "13:00:00", end: "14:00:00" },
        location: { building: null, buildingDescription: "Lab", room: "101", campus: null },
      }),
    ];
    const result = formatMeetingTimesTooltip(mts);
    expect(result).toContain("Mondays, Wednesdays & Fridays, 9:00–9:50 AM");
    expect(result).toContain("Thursdays, 1:00–2:00 PM\nLab 101");
    expect(result).toContain("\n\n");
  });
});

describe("formatInstructorName", () => {
  it("formats displayName with comma", () => {
    expect(formatInstructorName("Ramirez, Maria Elena")).toBe("Maria Elena Ramirez");
  });
  it("returns name as-is without comma", () => {
    expect(formatInstructorName("Staff")).toBe("Staff");
  });
  it("trims whitespace", () => {
    expect(formatInstructorName("  Smith , John  ")).toBe("John Smith");
  });
  it("handles last-name-only after comma", () => {
    expect(formatInstructorName("Solo,")).toBe("Solo");
  });
  it("uses firstName/lastName from InstructorResponse when available", () => {
    expect(
      formatInstructorName({
        displayName: "Ramirez, Maria Elena",
        firstName: "Maria",
        lastName: "Ramirez",
      })
    ).toBe("Maria Ramirez");
  });
  it("falls back to displayName when firstName is null", () => {
    expect(
      formatInstructorName({
        displayName: "Ramirez, Maria Elena",
        firstName: null,
        lastName: "Ramirez",
      })
    ).toBe("Maria Elena Ramirez");
  });
  it("falls back to displayName when lastName is null", () => {
    expect(
      formatInstructorName({
        displayName: "Ramirez, Maria Elena",
        firstName: "Maria",
        lastName: null,
      })
    ).toBe("Maria Elena Ramirez");
  });
  it("falls back to displayName when both are null", () => {
    expect(formatInstructorName({ displayName: "Staff", firstName: null, lastName: null })).toBe(
      "Staff"
    );
  });
});

function makeCourse(overrides: Partial<CourseResponse> = {}): CourseResponse {
  return {
    crn: "12345",
    subject: "CS",
    courseNumber: "1234",
    title: "Test Course",
    termCode: "202510",
    termSlug: "Spring-2025",
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
    expect(formatMeetingTimeSummary(course)).toBe("TTh TBA");
  });
});

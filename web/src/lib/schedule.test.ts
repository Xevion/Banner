import type { DbMeetingTime } from "$lib/bindings";
import {
  formatDuration,
  meetingDayFlags,
  meetingDurationMinutes,
  meetingTrackSpans,
  parseTimeMinutes,
} from "$lib/schedule";
import { describe, expect, it } from "vitest";

function makeMeetingTime(overrides: Partial<DbMeetingTime> = {}): DbMeetingTime {
  return {
    timeRange: null,
    dateRange: { start: "2024-08-26", end: "2024-12-12" },
    days: [],
    location: null,
    meetingType: "CLAS",
    meetingScheduleType: "LEC",
    ...overrides,
  };
}

describe("parseTimeMinutes", () => {
  it("parses midnight", () => expect(parseTimeMinutes("00:00:00")).toBe(0));
  it("parses 07:00:00", () => expect(parseTimeMinutes("07:00:00")).toBe(420));
  it("parses 14:30:00", () => expect(parseTimeMinutes("14:30:00")).toBe(870));
  it("parses without seconds", () => expect(parseTimeMinutes("14:30")).toBe(870));
  it("returns null for null", () => expect(parseTimeMinutes(null)).toBeNull());
  it("returns null for garbage", () => expect(parseTimeMinutes("nope")).toBeNull());
});

describe("formatDuration", () => {
  it("renders sub-hour durations in minutes", () => expect(formatDuration(50)).toBe("50 min"));
  it("renders whole hours without minutes", () => expect(formatDuration(360)).toBe("6h"));
  it("renders mixed durations", () => expect(formatDuration(75)).toBe("1h 15m"));
  it("renders exactly one hour", () => expect(formatDuration(60)).toBe("1h"));
});

describe("meetingDurationMinutes", () => {
  it("measures a 75-minute block", () => {
    const mt = makeMeetingTime({ timeRange: { start: "14:30:00", end: "15:45:00" } });
    expect(meetingDurationMinutes(mt)).toBe(75);
  });
  it("measures a 50-minute lecture", () => {
    const mt = makeMeetingTime({ timeRange: { start: "09:00:00", end: "09:50:00" } });
    expect(meetingDurationMinutes(mt)).toBe(50);
  });
  it("returns null when the time range is missing", () => {
    expect(meetingDurationMinutes(makeMeetingTime())).toBeNull();
  });
});

describe("meetingDayFlags", () => {
  it("returns all false for no meetings", () => {
    expect(meetingDayFlags([])).toEqual([false, false, false, false, false, false, false]);
  });
  it("flags TTh", () => {
    const mt = makeMeetingTime({ days: ["tuesday", "thursday"] });
    expect(meetingDayFlags([mt])).toEqual([false, true, false, true, false, false, false]);
  });
  it("unions days across meetings", () => {
    const lecture = makeMeetingTime({ days: ["monday", "wednesday", "friday"] });
    const lab = makeMeetingTime({ days: ["tuesday"] });
    expect(meetingDayFlags([lecture, lab])).toEqual([true, true, true, false, true, false, false]);
  });
  it("flags weekends", () => {
    const mt = makeMeetingTime({ days: ["saturday", "sunday"] });
    expect(meetingDayFlags([mt])).toEqual([false, false, false, false, false, true, true]);
  });
});

describe("meetingTrackSpans", () => {
  it("returns nothing when there is no time range", () => {
    expect(meetingTrackSpans([makeMeetingTime()])).toEqual([]);
  });

  it("maps a 2:30-3:45 PM meeting onto the 7am-10pm window", () => {
    const mt = makeMeetingTime({ timeRange: { start: "14:30:00", end: "15:45:00" } });
    const [span] = meetingTrackSpans([mt]);
    expect(span.left).toBeCloseTo(50, 5);
    expect(span.width).toBeCloseTo((75 / 900) * 100, 5);
  });

  it("starts at zero for a 7 AM meeting", () => {
    const mt = makeMeetingTime({ timeRange: { start: "07:00:00", end: "07:50:00" } });
    expect(meetingTrackSpans([mt])[0].left).toBeCloseTo(0, 5);
  });

  it("clamps a meeting that starts before the window", () => {
    const mt = makeMeetingTime({ timeRange: { start: "06:00:00", end: "07:30:00" } });
    const [span] = meetingTrackSpans([mt]);
    expect(span.left).toBe(0);
    expect(span.width).toBeCloseTo((30 / 900) * 100, 5);
  });

  it("clamps a meeting that runs past the window", () => {
    const mt = makeMeetingTime({ timeRange: { start: "21:30:00", end: "23:00:00" } });
    const [span] = meetingTrackSpans([mt]);
    expect(span.left).toBeCloseTo((870 / 900) * 100, 5);
    expect(span.left + span.width).toBeCloseTo(100, 5);
  });

  it("keeps a meeting fully outside the window visible at the nearest edge", () => {
    const mt = makeMeetingTime({ timeRange: { start: "05:00:00", end: "06:00:00" } });
    const [span] = meetingTrackSpans([mt]);
    expect(span.left).toBe(0);
    expect(span.width).toBeGreaterThan(0);
  });

  it("merges overlapping meetings into one span", () => {
    const a = makeMeetingTime({ timeRange: { start: "09:00:00", end: "10:00:00" } });
    const b = makeMeetingTime({ timeRange: { start: "09:30:00", end: "11:00:00" } });
    const spans = meetingTrackSpans([a, b]);
    expect(spans).toHaveLength(1);
    expect(spans[0].left).toBeCloseTo((120 / 900) * 100, 5);
    expect(spans[0].width).toBeCloseTo((120 / 900) * 100, 5);
  });

  it("keeps disjoint meetings as separate spans, sorted by start", () => {
    const afternoon = makeMeetingTime({ timeRange: { start: "14:00:00", end: "15:00:00" } });
    const morning = makeMeetingTime({ timeRange: { start: "09:00:00", end: "10:00:00" } });
    const spans = meetingTrackSpans([afternoon, morning]);
    expect(spans).toHaveLength(2);
    expect(spans[0].left).toBeLessThan(spans[1].left);
  });
});

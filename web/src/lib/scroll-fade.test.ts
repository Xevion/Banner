import { describe, it, expect } from "vitest";
import {
  FADE_DISTANCE,
  FADE_PERCENT,
  leftOpacity,
  rightOpacity,
  maskGradient,
  type ScrollMetrics,
} from "$lib/scroll-fade";

describe("leftOpacity", () => {
  it("returns 0 when scrollLeft is 0", () => {
    expect(leftOpacity({ scrollLeft: 0, scrollWidth: 1000, clientWidth: 500 })).toBe(0);
  });

  it("returns 1 when scrollLeft >= FADE_DISTANCE", () => {
    expect(leftOpacity({ scrollLeft: FADE_DISTANCE, scrollWidth: 1000, clientWidth: 500 })).toBe(1);
    expect(
      leftOpacity({ scrollLeft: FADE_DISTANCE + 50, scrollWidth: 1000, clientWidth: 500 })
    ).toBe(1);
  });

  it("returns proportional value for partial scroll", () => {
    const half = FADE_DISTANCE / 2;
    expect(leftOpacity({ scrollLeft: half, scrollWidth: 1000, clientWidth: 500 })).toBeCloseTo(0.5);
  });
});

describe("rightOpacity", () => {
  it("returns 0 when content fits (no scroll needed)", () => {
    expect(rightOpacity({ scrollLeft: 0, scrollWidth: 500, clientWidth: 500 })).toBe(0);
  });

  it("returns 0 when scrolled to the end", () => {
    expect(rightOpacity({ scrollLeft: 500, scrollWidth: 1000, clientWidth: 500 })).toBe(0);
  });

  it("returns 1 when far from the end", () => {
    expect(rightOpacity({ scrollLeft: 0, scrollWidth: 1000, clientWidth: 500 })).toBe(1);
  });

  it("returns proportional value near the end", () => {
    const maxScroll = 500; // scrollWidth(1000) - clientWidth(500)
    const remaining = FADE_DISTANCE / 2;
    const scrollLeft = maxScroll - remaining;
    expect(rightOpacity({ scrollLeft, scrollWidth: 1000, clientWidth: 500 })).toBeCloseTo(0.5);
  });
});

describe("maskGradient", () => {
  it("returns full transparent-to-transparent gradient when no scroll", () => {
    const metrics: ScrollMetrics = { scrollLeft: 0, scrollWidth: 500, clientWidth: 500 };
    const result = maskGradient(metrics);
    // leftOpacity=0, rightOpacity=0 → leftEnd=0%, rightStart=100%
    expect(result).toBe(
      "linear-gradient(to right, transparent 0%, black 0%, black 100%, transparent 100%)"
    );
  });

  it("includes fade zones when scrolled to the middle", () => {
    const metrics: ScrollMetrics = {
      scrollLeft: FADE_DISTANCE,
      scrollWidth: 1000,
      clientWidth: 500,
    };
    const result = maskGradient(metrics);
    // leftOpacity=1 → leftEnd=FADE_PERCENT%, rightOpacity=1 → rightStart=100-FADE_PERCENT%
    expect(result).toContain(`black ${FADE_PERCENT}%`);
    expect(result).toContain(`black ${100 - FADE_PERCENT}%`);
  });
});

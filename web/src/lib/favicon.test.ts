import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  B_PATH,
  FAVICON_STATE_COLORS,
  type FaviconState,
  faviconStateSvg,
  mostUrgentState,
  resetFavicon,
  setFaviconState,
} from "$lib/favicon";
import { beforeEach, describe, expect, it } from "vitest";

const staticSvg = (name: string) =>
  readFileSync(join(import.meta.dirname, "../../static", name), "utf8");

describe("mostUrgentState", () => {
  it("returns idle for no watches", () => {
    expect(mostUrgentState([])).toBe("idle");
  });

  it("prefers open over every other state", () => {
    expect(mostUrgentState(["full", "over", "low", "open"])).toBe("open");
  });

  it("orders the remaining states low, over, full", () => {
    expect(mostUrgentState(["full", "over", "low"])).toBe("low");
    expect(mostUrgentState(["full", "over"])).toBe("over");
    expect(mostUrgentState(["full", "idle"])).toBe("full");
  });
});

describe("faviconStateSvg", () => {
  it("paints the band with the state color", () => {
    for (const [state, color] of Object.entries(FAVICON_STATE_COLORS)) {
      expect(faviconStateSvg(state as FaviconState)).toContain(
        `<rect x="25" width="7" height="32" fill="${color}"/>`
      );
    }
  });

  it("carries the theme switch so the icon follows the OS", () => {
    const svg = faviconStateSvg("open");
    expect(svg).toContain("prefers-color-scheme: light");
    expect(svg).toContain("fill:var(--bg)");
    expect(svg).toContain("fill:var(--ink)");
  });

  it("parses as a single SVG root", () => {
    const doc = new DOMParser().parseFromString(faviconStateSvg("low"), "image/svg+xml");
    expect(doc.querySelector("parsererror")).toBeNull();
    expect(doc.documentElement.getAttribute("viewBox")).toBe("0 0 32 32");
  });
});

describe("setFaviconState", () => {
  beforeEach(() => {
    document.head.innerHTML =
      '<link rel="icon" href="/favicon.ico" sizes="32x32">' +
      '<link rel="icon" href="/favicon.svg" type="image/svg+xml">';
  });

  const svgLink = () =>
    document.querySelector<HTMLLinkElement>('link[rel="icon"][type="image/svg+xml"]');

  it("swaps the SVG icon for a percent-encoded data URI", () => {
    setFaviconState("full");
    const href = svgLink()?.href ?? "";
    expect(href.startsWith("data:image/svg+xml,")).toBe(true);
    expect(decodeURIComponent(href.slice("data:image/svg+xml,".length))).toBe(
      faviconStateSvg("full")
    );
  });

  it("leaves the ICO fallback alone", () => {
    setFaviconState("open");
    expect(document.querySelector<HTMLLinkElement>('link[sizes="32x32"]')?.href).toContain(
      "/favicon.ico"
    );
  });

  it("restores the static icon", () => {
    setFaviconState("over");
    resetFavicon();
    expect(svgLink()?.href).toContain("/favicon.svg");
  });

  it("does nothing when the page has no SVG icon link", () => {
    document.head.innerHTML = "";
    expect(() => setFaviconState("open")).not.toThrow();
  });
});

describe("shipped assets", () => {
  it("draw the same letterform as the runtime builder", () => {
    for (const name of ["favicon.svg", "favicon-state.svg"]) {
      const d = /<path[^>]*\sd="([^"]+)"/.exec(staticSvg(name))?.[1];
      expect(d, `${name} letterform`).toBe(B_PATH);
    }
  });

  it("default the state band to idle", () => {
    expect(staticSvg("favicon-state.svg")).toContain(`--state: ${FAVICON_STATE_COLORS.idle}`);
  });
});

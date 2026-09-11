import type { Preview } from "@storybook/sveltekit";
import { sb } from "storybook/test";
import { MINIMAL_VIEWPORTS } from "storybook/viewport";
import "../src/routes/layout.css";
import "overlayscrollbars/overlayscrollbars.css";
import TooltipDecorator from "./TooltipDecorator.svelte";
import ThemeDecorator from "./ThemeDecorator.svelte";
import AuthDecorator from "./AuthDecorator.svelte";

// Storybook has no backend; stories that fetch must stub the call they rely on.
sb.mock(import("../src/lib/api.ts"), { spy: true });

/**
 * The widths the app actually changes shape at, since a device name says nothing
 * about which `sm:` or `lg:` rules a component is under. Each is one pixel past
 * its Tailwind breakpoint, so picking it shows the layout that breakpoint turns
 * on; `below-sm` is the last width before the tables swap to their card layout.
 */
const breakpointViewports = {
  belowSm: {
    name: "Below sm (639px)",
    type: "mobile",
    styles: { width: "639px", height: "900px" },
  },
  sm: { name: "sm (641px)", type: "mobile", styles: { width: "641px", height: "900px" } },
  md: { name: "md (769px)", type: "tablet", styles: { width: "769px", height: "1000px" } },
  lg: { name: "lg (1025px)", type: "desktop", styles: { width: "1025px", height: "800px" } },
  xl: { name: "xl (1281px)", type: "desktop", styles: { width: "1281px", height: "800px" } },
  xl2: { name: "2xl (1537px)", type: "desktop", styles: { width: "1537px", height: "900px" } },
} as const;

const preview: Preview = {
  globalTypes: {
    theme: {
      description: "Color scheme",
      toolbar: {
        title: "Theme",
        icon: "circlehollow",
        items: [
          { value: "light", title: "Light", icon: "sun" },
          { value: "dark", title: "Dark", icon: "moon" },
        ],
        dynamicTitle: true,
      },
    },
    authMode: {
      description: "Auth state",
      toolbar: {
        title: "Auth",
        icon: "user",
        items: [
          { value: "unauthenticated", title: "Unauthenticated" },
          { value: "authenticated", title: "Authenticated" },
          { value: "admin", title: "Admin" },
          { value: "loading", title: "Loading" },
        ],
        dynamicTitle: true,
      },
    },
  },

  initialGlobals: {
    theme: "light",
    authMode: "unauthenticated",
  },

  decorators: [
    (storyFn) => {
      storyFn();
      return { Component: TooltipDecorator };
    },
    (storyFn, context) => {
      storyFn();
      return {
        Component: ThemeDecorator,
        props: { theme: context.globals.theme ?? "light" },
      };
    },
    (storyFn, context) => {
      storyFn();
      return {
        Component: AuthDecorator,
        props: { authMode: context.globals.authMode ?? "unauthenticated" },
      };
    },
  ],

  parameters: {
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/i,
      },
    },

    layout: "centered",

    viewport: {
      options: { ...breakpointViewports, ...MINIMAL_VIEWPORTS },
    },

    a11y: {
      // 'todo' - show a11y violations in the test UI only
      // 'error' - fail CI on a11y violations
      // 'off' - skip a11y checks entirely
      test: "error",
      config: {
        rules: [{ id: "color-contrast", enabled: false }],
      },
    },
  },
};

export default preview;

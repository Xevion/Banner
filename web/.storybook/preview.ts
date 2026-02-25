import type { Preview } from "@storybook/sveltekit";
import "../src/routes/layout.css";
import "overlayscrollbars/overlayscrollbars.css";
import TooltipDecorator from "./TooltipDecorator.svelte";
import ThemeDecorator from "./ThemeDecorator.svelte";
import AuthDecorator from "./AuthDecorator.svelte";

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

    a11y: {
      // 'todo' - show a11y violations in the test UI only
      // 'error' - fail CI on a11y violations
      // 'off' - skip a11y checks entirely
      test: "todo",
    },
  },
};

export default preview;

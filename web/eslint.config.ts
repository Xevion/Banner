// For more info, see https://github.com/storybookjs/eslint-plugin-storybook#configuration-flat-config-format
import storybook from "eslint-plugin-storybook";

import path from "node:path";
import { includeIgnoreFile } from "@eslint/compat";
import js from "@eslint/js";
import * as customParser from "@xevion/ts-eslint-extra";
import svelte from "eslint-plugin-svelte";
import globals from "globals";
import tseslint from "typescript-eslint";
import svelteConfig from "./svelte.config.ts";

const gitignorePath = path.resolve(import.meta.dirname, ".gitignore");

export default tseslint.config(
  includeIgnoreFile(gitignorePath),
  {
    ignores: [
      "dist/",
      ".svelte-kit/",
      "build/",
      "src/lib/bindings/",
      ".storybook/",
      "src/**/*.stories.svelte",
      "src/**/*.stories.ts",
    ],
  },
  // Base JS rules
  js.configs.recommended,
  // TypeScript: recommended type-checked + stylistic type-checked
  ...tseslint.configs.strictTypeChecked,
  ...tseslint.configs.stylisticTypeChecked,
  // Svelte recommended
  ...svelte.configs.recommended,
  // Global settings: environments + shared rules
  {
    languageOptions: {
      globals: { ...globals.browser, ...globals.node },
      parserOptions: {
        project: "./tsconfig.json",
        tsconfigRootDir: import.meta.dirname,
        extraFileExtensions: [".svelte"],
      },
    },
    rules: {
      // typescript-eslint recommends disabling no-undef for TS projects
      // see: https://typescript-eslint.io/troubleshooting/faqs/eslint/#i-get-errors-from-the-no-undef-rule-about-global-variables-not-being-defined-even-though-there-are-no-typescript-errors
      "no-undef": "off",
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
      "@typescript-eslint/consistent-type-imports": [
        "error",
        { prefer: "type-imports", fixStyle: "separate-type-imports" },
      ],
      "@typescript-eslint/restrict-template-expressions": ["error", { allowNumber: true }],
      "@typescript-eslint/no-confusing-void-expression": ["error", { ignoreArrowShorthand: true }],
    },
  },
  // TS files: use custom parser to resolve .svelte named exports
  {
    files: ["**/*.ts"],
    languageOptions: {
      parser: customParser,
    },
  },
  // Svelte files: svelte-eslint-parser (from svelte.configs.recommended) as
  // outer parser, with custom parser for script blocks to resolve .svelte exports
  {
    files: ["**/*.svelte", "**/*.svelte.ts"],
    languageOptions: {
      parserOptions: {
        parser: customParser,
        svelteConfig,
      },
    },
    rules: {
      "svelte/no-navigation-without-resolve": "off",
      // The Svelte parser reports every {@render snippet()} in markup as a void
      // expression, and a render tag cannot be hoisted to its own statement.
      "@typescript-eslint/no-confusing-void-expression": "off",
    },
  },
  // Svelte markup files only: $bindable() must sit in the destructuring default
  // slot, so the rule flags every bindable prop whose type is not optional.
  {
    files: ["**/*.svelte"],
    rules: {
      "@typescript-eslint/no-useless-default-assignment": "off",
    },
  },
  // Config and standalone scripts sit outside the tsconfig project.
  {
    files: ["*.config.ts", "scripts/**/*.ts", "console-logger.ts"],
    ...tseslint.configs.disableTypeChecked,
  },
  storybook.configs["flat/recommended"]
);

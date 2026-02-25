import { OverlayScrollbars, type PartialOptions } from "overlayscrollbars";
import { onMount } from "svelte";

/**
 * Set up OverlayScrollbars on an element.
 *
 * Theme colors are handled purely via CSS custom properties on `:root` / `.dark` (see layout.css).
 *
 * Must be called during component initialization (uses `onMount` internally).
 */
export function useOverlayScrollbars(getElement: () => HTMLElement, options: PartialOptions = {}) {
  onMount(() => {
    const element = getElement();
    const osInstance = OverlayScrollbars(element, {
      ...options,
      scrollbars: {
        ...options.scrollbars,
        theme: "os-theme-banner",
      },
    });

    return () => {
      osInstance.destroy();
    };
  });
}

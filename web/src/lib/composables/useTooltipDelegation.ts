/**
 * Event-delegated singleton tooltip for table cells.
 *
 * Cells opt in via `data-tooltip="text"` attributes. One tooltip element is
 * lazily created and reused for every hover -- zero per-cell component overhead.
 *
 * Optional attributes:
 *  - `data-tooltip-side`  -- placement (default "top")
 *  - `data-tooltip-delay` -- show delay in ms (default 150)
 */
import { type Placement, computePosition, flip, offset, shift } from "@floating-ui/dom";

const TOOLTIP_CLASS =
  "fixed z-50 bg-card text-card-foreground text-xs border border-border " +
  "rounded-md px-2.5 py-1.5 shadow-sm whitespace-pre-line max-w-max text-left " +
  "pointer-events-none transition-opacity duration-100";

const DEFAULT_DELAY = 150;
const HIDE_DELAY = 60;

export function useTooltipDelegation(container: HTMLElement) {
  let tooltipEl: HTMLDivElement | null = null;
  let currentTarget: HTMLElement | null = null;
  let showTimeoutId: ReturnType<typeof setTimeout> | undefined;
  let hideTimeoutId: ReturnType<typeof setTimeout> | undefined;

  function getOrCreateTooltip(): HTMLDivElement {
    if (!tooltipEl) {
      tooltipEl = document.createElement("div");
      tooltipEl.className = TOOLTIP_CLASS;
      tooltipEl.style.opacity = "0";
      tooltipEl.setAttribute("role", "tooltip");
      document.body.appendChild(tooltipEl);
    }
    return tooltipEl;
  }

  function showTooltip(target: HTMLElement) {
    const text = target.dataset.tooltip;
    if (!text) return;

    clearTimeout(hideTimeoutId);

    // If already showing for a different target, switch instantly
    const instant = currentTarget !== null && currentTarget !== target;
    if (currentTarget === target && tooltipEl?.style.opacity === "1") return;

    currentTarget = target;
    clearTimeout(showTimeoutId);

    const side = (target.dataset.tooltipSide as Placement) ?? "top";
    const delay = Number.parseInt(target.dataset.tooltipDelay ?? String(DEFAULT_DELAY), 10);

    const doShow = () => {
      const el = getOrCreateTooltip();
      el.textContent = text;
      el.style.opacity = "1";

      void computePosition(target, el, {
        strategy: "fixed",
        placement: side,
        middleware: [offset(6), flip(), shift({ padding: 8 })],
      }).then(({ x, y }) => {
        // Guard: target may have changed while awaiting
        if (currentTarget !== target) return;
        el.style.left = `${x}px`;
        el.style.top = `${y}px`;
      });
    };

    if (instant) {
      doShow();
    } else {
      showTimeoutId = setTimeout(doShow, delay);
    }
  }

  function hideTooltip() {
    clearTimeout(showTimeoutId);
    hideTimeoutId = setTimeout(() => {
      if (tooltipEl) tooltipEl.style.opacity = "0";
      currentTarget = null;
    }, HIDE_DELAY);
  }

  function onMouseOver(e: MouseEvent) {
    const target = (e.target as HTMLElement).closest?.("[data-tooltip]");
    if (target instanceof HTMLElement) {
      showTooltip(target);
    } else if (currentTarget) {
      hideTooltip();
    }
  }

  function onMouseLeave() {
    hideTooltip();
  }

  function onFocusIn(e: FocusEvent) {
    const target = (e.target as HTMLElement).closest?.("[data-tooltip]");
    if (target instanceof HTMLElement) showTooltip(target);
  }

  function onFocusOut() {
    hideTooltip();
  }

  container.addEventListener("mouseover", onMouseOver);
  container.addEventListener("mouseleave", onMouseLeave);
  container.addEventListener("focusin", onFocusIn);
  container.addEventListener("focusout", onFocusOut);

  return {
    destroy() {
      container.removeEventListener("mouseover", onMouseOver);
      container.removeEventListener("mouseleave", onMouseLeave);
      container.removeEventListener("focusin", onFocusIn);
      container.removeEventListener("focusout", onFocusOut);
      clearTimeout(showTimeoutId);
      clearTimeout(hideTimeoutId);
      if (tooltipEl) {
        tooltipEl.remove();
        tooltipEl = null;
      }
    },
  };
}

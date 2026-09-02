/**
 * Mirror a boolean, but only once it has stayed true for `delayMs`.
 *
 * A search that resolves in a few milliseconds never flips it, so the common
 * fast path renders the new rows directly instead of dimming and undimming on
 * the way there. Falls back to false the moment the source does.
 */
export function useDelayedFlag(getValue: () => boolean, delayMs = 150) {
  let raised = $state(false);

  $effect(() => {
    if (!getValue()) {
      raised = false;
      return;
    }
    const timer = setTimeout(() => {
      raised = true;
    }, delayMs);
    return () => clearTimeout(timer);
  });

  return {
    get current() {
      return raised;
    },
  };
}

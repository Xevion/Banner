export function useDebounceSearch(onSearch: (q: string) => void, delay = 300) {
  let input = $state("");
  let timeout: ReturnType<typeof setTimeout> | undefined;

  function trigger() {
    clearTimeout(timeout);
    timeout = setTimeout(() => onSearch(input), delay);
  }

  function clear(callback?: () => void) {
    clearTimeout(timeout);
    input = "";
    onSearch("");
    callback?.();
  }

  $effect(() => () => clearTimeout(timeout));

  return {
    get input() {
      return input;
    },
    set input(v) {
      input = v;
    },
    trigger,
    clear,
  };
}

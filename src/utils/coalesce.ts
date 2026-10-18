/** Minimum gap between focus-triggered refreshes. Each refresh may spawn git processes. */
export const FOCUS_REFRESH_GAP_MS = 2000;

/**
 * Limits how often `fn` runs. The first call runs immediately. Calls that arrive inside
 * `minGapMs` are merged into one trailing run at the end of the gap, so the final state
 * is always refreshed without a burst of work (for example a focus event followed by
 * visibility and online events).
 */
export function coalesce(fn: () => void, minGapMs: number): { call: () => void; cancel: () => void } {
  let last = Number.NEGATIVE_INFINITY;
  let timer: ReturnType<typeof setTimeout> | null = null;

  const run = () => {
    timer = null;
    last = Date.now();
    fn();
  };

  const call = () => {
    if (timer !== null) return;
    const wait = last + minGapMs - Date.now();
    if (wait <= 0) run();
    else timer = setTimeout(run, wait);
  };

  const cancel = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
  };

  return { call, cancel };
}

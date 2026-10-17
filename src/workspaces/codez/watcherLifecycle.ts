/** Release only after a successful start, including when unmounted during start. */
export function startOwnedWatcher(
  start: () => Promise<void>,
  stop: () => Promise<void>,
): () => void {
  const ready = Promise.resolve().then(start).then(() => true, () => false);
  let disposed = false;
  return () => {
    if (disposed) return;
    disposed = true;
    void ready.then((started) => {
      if (started) return stop().catch(() => {});
    });
  };
}

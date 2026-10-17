/** Serialize create/cleanup for a tab ID, including StrictMode effect replay. */
const backendIds = new Map<string, string>();
export const terminalBackendId = (id: string) => backendIds.get(id);
const tails = new Map<string, Promise<void>>();
export function acquireTerminal(id: string, create: () => Promise<void>, destroy: () => Promise<void>, backendId = id) {
  let disposed = false;
  let created = false;
  const enqueue = (operation: () => Promise<void>) => {
    const next = (tails.get(id) ?? Promise.resolve()).then(operation);
    const settled = next.catch(() => {});
    tails.set(id, next);
    // A failed cleanup poisons this ID; never create a replacement over it.
    void next.then(() => { if (tails.get(id) === next) tails.delete(id); }, () => {});
    void settled;
    return next;
  };
  const ready = enqueue(async () => {
    if (disposed) return;
    backendIds.set(id, backendId);
    await create();
    created = true;
    if (disposed) { await destroy(); created = false; if (backendIds.get(id) === backendId) backendIds.delete(id); }
  });
  let cleanup: Promise<void> | undefined;
  return {
    ready,
    dispose() {
      disposed = true;
      cleanup ??= enqueue(async () => { if (created) { await destroy(); created = false; } if (backendIds.get(id) === backendId) backendIds.delete(id); });
      return cleanup;
    },
  };
}

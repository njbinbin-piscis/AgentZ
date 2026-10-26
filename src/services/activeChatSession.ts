// Tiny pub/sub holding the id of the chat session currently shown in the
// assistant panel, so sibling UI (status bar) can show per-session usage.
let current: string | null = null;
const listeners = new Set<() => void>();

export function setActiveChatSession(id: string | null): void {
  if (current === id) return;
  current = id;
  listeners.forEach((l) => l());
}

export function getActiveChatSession(): string | null {
  return current;
}

export function subscribeActiveChatSession(l: () => void): () => void {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}
// Which background task is shown in the bottom-panel "background output"
// tab. `nonce` changes on every request so re-clicking reopens the panel.
export interface BgTerminalRequest {
  taskId: string;
  nonce: number;
}

let current: BgTerminalRequest | null = null;
let seq = 0;
const listeners = new Set<() => void>();

export function openBgTerminal(taskId: string): void {
  seq += 1;
  current = { taskId, nonce: seq };
  listeners.forEach((l) => l());
}

export function getBgTerminal(): BgTerminalRequest | null {
  return current;
}

export function subscribeBgTerminal(l: () => void): () => void {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}
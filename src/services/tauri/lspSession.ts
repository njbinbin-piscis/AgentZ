// Shared LSP session pool.
//
// One language server (bridge) is enough per `project + language`, so tabs that
// open files of the same language reuse a single connection instead of spawning
// a client each. Sessions are reference-counted: the connection is torn down and
// the backend session stopped once the last consumer releases it.

import { LspClient, lspApi } from "./lsp";

const sessions = new Map<string, Session>();

function keyFor(projectDir: string, language: string): string {
  return `${projectDir}::${language}`;
}

// Consecutive failed restart attempts allowed for one crash episode. A healthy
// reconnect resets this budget, so a later crash is recovered again.
const MAX_RESTART_ATTEMPTS = 3;
// Hard cap on crash recoveries per window, so a start-then-crash loop (where each
// reconnect succeeds) still stops.
const MAX_RECOVERIES_PER_WINDOW = 10;
const RECOVERY_WINDOW_MS = 5 * 60_000;

class Session {
  refCount = 0;
  private client: LspClient | null = null;
  private started: Promise<LspClient>;
  private disposed = false;
  private recovering = false;
  private recoveryAttempts = 0;
  private recoveryStarts: number[] = [];
  private established = false;

  constructor(
    readonly key: string,
    private projectDir: string,
    private language: string,
  ) {
    // Kick off start+connect immediately so concurrent acquires for the same
    // key share one handshake.
    this.started = (async () => {
      const port = await lspApi.start(projectDir, language);
      const client = new LspClient(port, () => { if (this.established) void this.recover(); });
      this.client = client;
      await client.connect(projectDir, language);
      this.established = true;
      return client;
    })();
  }

  private async recover(): Promise<void> {
    if (this.disposed || this.recovering || !this.client || this.refCount <= 0) return;
    this.recovering = true;
    try {
      const now = Date.now();
      this.recoveryStarts = this.recoveryStarts.filter((t) => now - t < RECOVERY_WINDOW_MS);
      if (this.recoveryStarts.length >= MAX_RECOVERIES_PER_WINDOW) {
        this.client.setRecoveryHealth("unavailable");
        console.warn("[LSP] Recovery window exhausted");
        return;
      }
      this.recoveryStarts.push(now);
      while (!this.disposed && this.recoveryAttempts < MAX_RESTART_ATTEMPTS) {
        this.client.setRecoveryHealth("recovering");
        this.recoveryAttempts += 1;
        await new Promise((resolve) => setTimeout(resolve, 100 * 2 ** (this.recoveryAttempts - 1)));
        if (this.disposed) return;
        try {
          const port = await lspApi.start(this.projectDir, this.language);
          if (this.disposed) {
            if (!sessions.has(this.key)) await lspApi.stop(this.projectDir, this.language).catch(() => {});
            return;
          }
          await this.client.reconnect(port, this.projectDir, this.language);
          // Healthy handshake: the next crash gets a fresh restart budget.
          this.recoveryAttempts = 0;
          return;
        } catch (error) {
          if (this.recoveryAttempts >= MAX_RESTART_ATTEMPTS) {
            this.client.setRecoveryHealth("unavailable");
            console.warn("[LSP] Recovery exhausted", error);
          }
        }
      }
    } finally {
      this.recovering = false;
    }
  }

  async acquire(): Promise<LspClient> {
    this.refCount += 1;
    try {
      return await this.started;
    } catch (err) {
      this.refCount -= 1;
      throw err;
    }
  }

  release(): boolean {
    this.refCount -= 1;
    return this.refCount <= 0;
  }

  dispose() {
    this.disposed = true;
    this.client?.disconnect();
  }
}

export interface LspSessionHandle {
  client: LspClient;
  /** Idempotent: releases this consumer's reference to the shared session. */
  release: () => void;
}

/**
 * Acquire a shared LSP session for `projectDir` + `language`, connecting on
 * first use. Rejects if the server is unavailable; the failed session is dropped
 * so a later attempt can retry.
 */
export async function acquireLspSession(
  projectDir: string,
  language: string,
): Promise<LspSessionHandle> {
  const key = keyFor(projectDir, language);
  let session = sessions.get(key);
  if (!session) {
    session = new Session(key, projectDir, language);
    sessions.set(key, session);
  }
  const active = session;

  let client: LspClient;
  try {
    client = await active.acquire();
  } catch (err) {
    // Drop the failed session so subsequent acquires retry from scratch.
    if (sessions.get(key) === active) {
      sessions.delete(key);
      active.dispose();
    }
    throw err;
  }

  let released = false;
  return {
    client,
    release: () => {
      if (released) return;
      released = true;
      if (active.release()) {
        if (sessions.get(key) === active) sessions.delete(key);
        active.dispose();
        lspApi.stop(projectDir, language).catch(() => {
          // Backend stop is best-effort; a stale session will be reused/restarted.
        });
      }
    },
  };
}

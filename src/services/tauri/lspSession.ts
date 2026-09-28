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

class Session {
  refCount = 0;
  private client: LspClient | null = null;
  private started: Promise<LspClient>;

  constructor(
    readonly key: string,
    projectDir: string,
    language: string,
  ) {
    // Kick off start+connect immediately so concurrent acquires for the same
    // key share one handshake.
    this.started = (async () => {
      const port = await lspApi.start(projectDir, language);
      const client = new LspClient(port);
      this.client = client;
      await client.connect(projectDir, language);
      return client;
    })();
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
    if (sessions.get(key) === active) sessions.delete(key);
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

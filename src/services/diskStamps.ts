// Digest of the bytes last read from / written to disk, keyed by absolute path.
// Shared between the CodeZ editor (Ctrl+S) and extension host document saves so that
// every write carries the expected digest and the backend can refuse stale overwrites.
export const diskStamps = new Map<string, string>();

export function tabNavigationTarget(ids: readonly string[], current: string, key: string): string | null {
  if (!ids.length) return null;
  const index = Math.max(0, ids.indexOf(current));
  if (key === "Home") return ids[0];
  if (key === "End") return ids[ids.length - 1];
  if (key === "ArrowRight") return ids[(index + 1) % ids.length];
  if (key === "ArrowLeft") return ids[(index + ids.length - 1) % ids.length];
  return null;
}

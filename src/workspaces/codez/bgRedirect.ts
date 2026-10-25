const NULL_SINKS = new Set(["nul", "$null", "/dev/null"]);

/**
 * Files a background command writes its own output into (`> out.log`,
 * `2>> err.txt`, `&> all.log`, `| Out-File x`, `| Tee-Object x`), resolved
 * against `cwd`. Null sinks and stream merges (`2>&1`) are skipped.
 */
export function redirectTargets(command: string, cwd: string): string[] {
  const found: string[] = [];
  const push = (raw: string) => {
    const target = raw.replace(/^["']|["']$/g, "");
    if (!target || target.startsWith("&") || NULL_SINKS.has(target.toLowerCase())) return;
    const abs = /^([a-zA-Z]:[\\/]|[\\/])/.test(target)
      ? target
      : `${cwd.replace(/[\\/]+$/, "")}/${target.replace(/^\.[\\/]/, "")}`;
    if (!found.includes(abs)) found.push(abs);
  };
  for (const m of command.matchAll(/(?:^|[\s;&|])(?:\d|&)?>>?\s*("[^"]+"|'[^']+'|[^\s&|;<>]+)/g)) push(m[1]);
  for (const m of command.matchAll(/\|\s*(?:Out-File|Tee-Object|tee)(?:\s+-(?:FilePath|Path|a|Append))*\s+("[^"]+"|'[^']+'|[^\s&|;]+)/gi)) {
    push(m[1]);
  }
  return found;
}

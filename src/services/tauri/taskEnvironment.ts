import { agentTaskApi, type AgentTaskInfo } from "./agentTask";

export const MISSING_TASK_ENVIRONMENT = "TASK_ENVIRONMENT_MISSING";

/** Resolve by stable task identity on every turn; never use the view's last worktree. */
export async function resolveTaskWorktree(args: {
  projectDir: string;
  taskId: string;
  isolate: boolean;
  isFree: boolean;
  resuming: boolean;
  base?: string;
  startMode?: "commit" | "working";
}): Promise<AgentTaskInfo | null> {
  const saved = await agentTaskApi.get(args.projectDir, args.taskId);
  if (saved) return saved;
  if (!args.isolate || args.isFree) return null;
  if (args.resuming) throw new Error(MISSING_TASK_ENVIRONMENT);
  return agentTaskApi.create(
    args.projectDir,
    args.taskId,
    args.base,
    args.startMode,
    args.startMode === "commit",
  );
}

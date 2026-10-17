import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useProjectScope } from "../../services/tauri/projectScope";
import { readTaskDetails, type TaskDetailSource } from "../../services/tauri/taskDetails";
import type { CurrentTaskRun } from "../../services/tauri/runDiscovery";
import { observeTaskView } from "../../services/tauri/taskView";
import { listTaskArtifacts } from "../../services/tauri/taskArtifacts";
import DeliverySummary, { type ArtifactObservation } from "./DeliverySummary";

/** Separate bounded observation sources. No acceptance, apply or publish action. */
export default function DeliveryReviewPanel({
  projectDir,
  sessionId,
  onReviewChanges,
  onOpenArtifacts,
}: {
  projectDir: string;
  sessionId: string;
  onReviewChanges?: () => void;
  onOpenArtifacts: () => void;
}) {
  const { i18n } = useTranslation();
  const zh = (i18n.resolvedLanguage ?? i18n.language ?? "").startsWith("zh");
  const text = (en: string, cn: string) => (zh ? cn : en);
  const scope = useProjectScope(projectDir);
  const key = JSON.stringify([projectDir, sessionId, scope?.handle ?? null]);
  const generation = useRef(0);
  const [refresh, setRefresh] = useState(0);
  const [state, setState] = useState<{
    key: string;
    loading: boolean;
    run: TaskDetailSource<CurrentTaskRun>;
    artifacts: ArtifactObservation;
  }>({ key, loading: !!scope, run: { status: "unknown" }, artifacts: { status: "unknown" } });
  const visible =
    state.key === key
      ? state
      : {
          key,
          loading: !!scope,
          run: { status: "unknown" } as TaskDetailSource<CurrentTaskRun>,
          artifacts: { status: "unknown" } as ArtifactObservation,
        };
  useEffect(() => {
    const gen = ++generation.current;
    setState({
      key,
      loading: !!scope,
      run: { status: "unknown" },
      artifacts: { status: scope ? "unknown" : "unavailable" },
    });
    if (!scope) return;
    void (async () => {
      const results = await Promise.allSettled([
        readTaskDetails(scope.handle, sessionId),
        (async () => {
          const view = await observeTaskView(scope.handle, sessionId, scope.project_dir);
          if (generation.current !== gen) return null;
          return listTaskArtifacts(scope.handle, view, null, 200);
        })(),
      ]);
      if (generation.current !== gen) return;
      const details = results[0],
        list = results[1];
      const run: TaskDetailSource<CurrentTaskRun> =
        details.status === "fulfilled" ? details.value.sources.run : { status: "unavailable" };
      let artifacts: ArtifactObservation =
        list.status === "fulfilled" && list.value
          ? ({
              status: list.value.store_missing ? "missing" : "available",
              ...(list.value.store_missing ? {} : { value: list.value }),
            } as ArtifactObservation)
          : { status: "unavailable" };
      if (
        details.status === "fulfilled" &&
        list.status === "fulfilled" &&
        list.value &&
        (details.value.scope.project_id !== list.value.scope.project_id ||
          details.value.scope.task_id !== list.value.scope.task_id ||
          details.value.scope.environment_id !== list.value.scope.environment_id)
      )
        artifacts = { status: "stale" };
      setState({ key, loading: false, run, artifacts });
    })();
    return () => {
      generation.current += 1;
    };
  }, [key, scope, sessionId, refresh]);
  return (
    <section role="region" aria-label={text("Delivery review", "交付评审")}>
      <p>
        {text("Project: ", "项目：")}
        {projectDir} · {text("Task: ", "任务：")}
        {sessionId}
      </p>
      <button disabled={!scope || visible.loading} onClick={() => setRefresh((value) => value + 1)}>
        {text("Refresh delivery observations", "刷新交付观察记录")}
      </button>
      {visible.loading && (
        <p role="status">{text("Reading independent sources…", "正在读取各个来源…")}</p>
      )}
      {!scope && (
        <p>{text("Authorized project access is unavailable.", "项目授权访问暂不可用。")}</p>
      )}
      <DeliverySummary
        run={visible.run}
        artifacts={visible.artifacts}
        onReviewChanges={onReviewChanges}
        onOpenArtifacts={onOpenArtifacts}
      />
    </section>
  );
}

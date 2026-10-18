import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useProjectScope } from "../../services/tauri/projectScope";
import { readCurrentTaskRun, type CurrentTaskRun } from "../../services/tauri/runDiscovery";
import {
  readRunEvents,
  RunEventCollector,
  type DurableRunEvent,
  type RunEventStream,
} from "../../services/tauri/runEvents";
import { coalesce, FOCUS_REFRESH_GAP_MS } from "../../utils/coalesce";
import "./RunHistoryPanel.css";
import { committedHighWatermark, committedNoticeBytes } from "../../services/tauri/runEventNotice";
import { onChatEvent, type ChatEventEnvelope } from "../../services/tauri/chat";
import {
  projectRunAgentEvents,
  type PassiveRunProjection,
} from "../../services/tauri/runEventProjection";

const english: Record<string, string> = {
  queued: "Queued",
  running: "Running",
  cancelling: "Stopping",
  succeeded: "Succeeded",
  failed: "Failed",
  cancelled: "Cancelled",
  interrupted: "Interrupted",
  not_ready: "Delivery not ready",
  awaiting_review: "Awaiting review",
  accepted: "Accepted",
  rejected: "Rejected",
};
const chinese: Record<string, string> = {
  queued: "已排队",
  running: "执行中",
  cancelling: "正在停止",
  succeeded: "执行成功",
  failed: "执行失败",
  cancelled: "已取消",
  interrupted: "已中断",
  not_ready: "尚未交付",
  awaiting_review: "待评审",
  accepted: "已验收",
  rejected: "未验收",
};

/** Passive current-Run lifecycle history. Refresh never starts or resumes execution. */
export default function RunHistoryPanel({
  projectDir,
  sessionId,
  busy,
}: {
  projectDir: string | null;
  sessionId: string | null;
  busy: boolean;
}) {
  const { i18n } = useTranslation();
  const zh = (i18n.resolvedLanguage ?? i18n.language ?? "").startsWith("zh");
  const label = (en: string, cn: string) => (zh ? cn : en);
  const states = zh ? chinese : english;
  const scope = useProjectScope(projectDir);
  const requestKey = JSON.stringify([scope?.handle ?? null, sessionId]);
  const [view, setView] = useState<{
    key: string;
    run: CurrentTaskRun | null;
    events: DurableRunEvent[];
    preview: PassiveRunProjection | null;
    loading: boolean;
    error: string | null;
  }>({
    key: requestKey,
    run: null,
    events: [],
    preview: null,
    loading: Boolean(scope && sessionId),
    error: null,
  });
  // Hide old-task state during render, before passive effect cleanup can execute.
  const { run, events, preview, loading, error } =
    view.key === requestKey
      ? view
      : {
          run: null,
          events: [],
          preview: null,
          loading: Boolean(scope && sessionId),
          error: null,
        };
  const reload = useRef<() => void>(() => {});
  const sequence = useRef(0);
  const requestRefresh = useCallback(() => reload.current(), []);

  useEffect(() => {
    // Focus, online, and visibility events often arrive together; one reload covers the burst.
    const refresh = coalesce(requestRefresh, FOCUS_REFRESH_GAP_MS);
    const visible = () => {
      if (document.visibilityState === "visible") refresh.call();
    };
    window.addEventListener("focus", refresh.call);
    window.addEventListener("online", refresh.call);
    document.addEventListener("visibilitychange", visible);
    return () => {
      refresh.cancel();
      window.removeEventListener("focus", refresh.call);
      window.removeEventListener("online", refresh.call);
      document.removeEventListener("visibilitychange", visible);
    };
  }, [requestRefresh]);

  useEffect(() => {
    const request = ++sequence.current;
    setView({
      key: requestKey,
      run: null,
      events: [],
      preview: null,
      error: null,
      loading: Boolean(scope && sessionId),
    });
    if (!scope || !sessionId) return;
    let stopped = false;
    let ready = false;
    let inFlight = false;
    let pending = false;
    let timer: ReturnType<typeof setTimeout> | null = null;
    let unlisten: (() => void) | null = null;
    let boundStream: RunEventStream | null = null;
    let observed = 0;
    let noticeFault: unknown = null;
    let highWater = 0;
    const earlyNotices: ChatEventEnvelope[] = [];
    let earlyNoticeBytes = 0;
    let committedCollector: RunEventCollector | null = null;
    let committedRun: CurrentTaskRun | null = null;
    let committedStream: string | null = null;
    let rediscover = true;
    const valid = () => !stopped && sequence.current === request;
    const fail = (cause: unknown) => {
      if (valid())
        setView({
          key: requestKey,
          run: null,
          events: [],
          preview: null,
          error: String(cause),
          loading: false,
        });
    };
    const schedule = () => {
      if (!valid() || !ready || timer !== null || inFlight) return;
      timer = setTimeout(() => {
        timer = null;
        void read();
      }, 200);
    };
    const read = async () => {
      if (!valid() || !ready) return;
      if (inFlight) {
        pending = true;
        return;
      }
      inFlight = true;
      pending = false;
      const discover = rediscover;
      rediscover = false;
      setView((previous) =>
        previous.key === requestKey ? { ...previous, loading: true } : previous,
      );
      try {
        if (noticeFault !== null) throw noticeFault;
        const current =
          discover || !committedRun || committedStream !== JSON.stringify(boundStream)
            ? await readCurrentTaskRun(scope.handle, sessionId)
            : committedRun;
        if (sequence.current !== request) return;
        if (!current) {
          boundStream = null;
          committedCollector = null;
          committedRun = null;
          committedStream = null;
          setView({
            key: requestKey,
            run: null,
            events: [],
            preview: null,
            error: null,
            loading: false,
          });
          highWater = 0;
          earlyNotices.length = 0;
          return;
        }
        const stream = {
          task_id: current.task_id,
          run_id: current.run_id,
          attempt_id: current.attempt_id,
          environment_id: current.environment_id,
        };
        if (JSON.stringify(boundStream) !== JSON.stringify(stream)) {
          highWater = 0;
          observed = 0;
        }
        boundStream = stream;
        for (const notice of earlyNotices)
          highWater = committedHighWatermark(stream, notice, null, highWater) ?? highWater;
        earlyNotices.length = 0;
        earlyNoticeBytes = 0;
        const requiredHighWater = highWater;
        const streamKey = JSON.stringify(stream);
        const collector =
          committedCollector && committedStream === streamKey
            ? committedCollector.fork()
            : new RunEventCollector(stream, null, 100);
        let complete = false;
        const verifiedPrefix = collector.snapshot();
        let eventCount = verifiedPrefix.length;
        let historyBytes = new TextEncoder().encode(JSON.stringify(verifiedPrefix)).byteLength;
        for (let page = 0; page < 100; page += 1) {
          const result = await readRunEvents(
            scope.handle,
            sessionId,
            current.run_id,
            current.attempt_id,
            collector.cursor(),
            200,
          );
          if (sequence.current !== request) return;
          const previous = collector.cursor()?.sequence ?? 0;
          const added = new Set<number>();
          for (const event of result.events) {
            if (event.cursor.sequence <= previous || added.has(event.cursor.sequence)) continue;
            added.add(event.cursor.sequence);
            historyBytes +=
              new TextEncoder().encode(JSON.stringify(event)).byteLength + (eventCount ? 1 : 0);
            eventCount += 1;
          }
          if (eventCount > 5000 || historyBytes > 8 * 1024 * 1024)
            throw new Error("Execution history exceeds the display budget");
          collector.append(result);
          if (!result.has_more) {
            complete = true;
            break;
          }
        }
        if (!complete) throw new Error("Lifecycle history exceeds the bounded page budget");
        if (noticeFault !== null) throw noticeFault;
        const capturedEvents = collector.snapshot();
        const cursor = collector.cursor();
        const nextObserved = cursor?.sequence ?? 0;
        if (nextObserved < requiredHighWater)
          throw new Error("Recorded history did not reach the committed event notice");
        earlyNotices.length = 0;
        const projection = projectRunAgentEvents(stream, capturedEvents);
        observed = nextObserved;
        if (observed >= highWater) highWater = 0;
        committedCollector = collector;
        committedRun = current;
        committedStream = streamKey;
        setView({
          key: requestKey,
          run: current,
          events: capturedEvents,
          preview: projection,
          error: null,
          loading: false,
        });
      } catch (cause: unknown) {
        pending = false;
        highWater = 0;
        earlyNotices.length = 0;
        fail(cause);
      } finally {
        inFlight = false;
        if (valid()) {
          setView((previous) =>
            previous.key === requestKey ? { ...previous, loading: false } : previous,
          );
          if (pending) schedule();
        }
      }
    };
    reload.current = () => {
      if (!valid()) return;
      pending = true;
      rediscover = true;
      if (!ready || inFlight) return;
      noticeFault = null;
      if (timer !== null) {
        clearTimeout(timer);
        timer = null;
      }
      void read();
    };
    void (async () => {
      try {
        const dispose = await onChatEvent((notice) => {
          if (
            !valid() ||
            notice.channel !== "agent_event_committed" ||
            notice.sessionId !== sessionId
          )
            return;
          try {
            if (boundStream === null) {
              // A notice arriving during initial discovery forces one follow-up replay.
              // Keep a bounded sample for authoritative stream validation; never consume its text.
              const bytes = committedNoticeBytes(notice);
              if (earlyNotices.length < 32 && earlyNoticeBytes + bytes <= 8 * 1024 * 1024) {
                earlyNotices.push(notice);
                earlyNoticeBytes += bytes;
              }
              pending = true;
              return;
            }
            const next = committedHighWatermark(boundStream, notice, null, highWater);
            if (next === null || next <= Math.max(observed, highWater)) return;
            highWater = next;
            pending = true;
            schedule();
          } catch (cause: unknown) {
            noticeFault = cause;
            pending = false;
            if (timer !== null) {
              clearTimeout(timer);
              timer = null;
            }
            fail(cause);
          }
        });
        if (!valid()) {
          dispose();
          return;
        }
        unlisten = dispose;
        ready = true;
        await read();
      } catch (cause: unknown) {
        fail(cause);
      }
    })();
    return () => {
      stopped = true;
      sequence.current += 1;
      if (timer !== null) clearTimeout(timer);
      unlisten?.();
      reload.current = () => {};
    };
  }, [scope, sessionId, requestKey]);

  useEffect(() => {
    requestRefresh();
  }, [busy, requestRefresh]);

  function eventLabel(event: DurableRunEvent): string {
    const payload =
      event.payload && typeof event.payload === "object"
        ? (event.payload as Record<string, unknown>)
        : {};
    const stateLabel = (value: unknown, fallback: string) =>
      typeof value === "string" && Object.prototype.hasOwnProperty.call(states, value)
        ? states[value]
        : fallback;
    switch (event.kind) {
      case "run.created":
        return label("Execution queued", "执行已排队");
      case "run.retry":
        return label("Retry queued", "重试已排队");
      case "run.execution":
        return stateLabel(payload.execution, label("Execution state recorded", "执行状态已记录"));
      case "run.delivery":
        return stateLabel(payload.delivery, label("Delivery decision recorded", "交付决定已记录"));
      default:
        return label("Lifecycle change recorded", "生命周期变化已记录");
    }
  }
  const lifecycleEvents = events.filter((event) => event.kind.startsWith("run."));
  if (!scope || !sessionId) return null;
  return (
    <section
      className="agentz-run-history"
      aria-label={label("Current execution history", "当前执行历史")}
    >
      <div className="agentz-run-history-heading">
        <strong>{label("Current execution history", "当前执行历史")}</strong>
        <button disabled={loading} onClick={requestRefresh}>
          {label("Refresh history", "刷新历史")}
        </button>
      </div>
      {loading && (
        <p role="status">{label("Reading recorded history…", "正在读取已记录的历史…")}</p>
      )}
      {error && <p role="alert">{error}</p>}
      {!loading && !error && !run && (
        <p>{label("No recorded execution for this task.", "此任务暂无执行记录。")}</p>
      )}
      {run && (
        <>
          <p>
            {states[run.execution]} · {states[run.delivery]}
          </p>
          {events.length === 0 && (
            <p>
              {label(
                "No captured events. Older executions may predate event capture.",
                "暂无事件记录；较早的执行可能发生在事件记录启用之前。",
              )}
            </p>
          )}
          {preview && events.some((event) => event.kind === "agent.event") && (
            <div className="agentz-run-preview">
              <strong>{label("Current execution event preview", "当前执行事件预览")}</strong>
              <p>
                {label(
                  "Read-only current attempt; refresh does not resume execution. This preview is not the complete conversation.",
                  "仅查看当前尝试；刷新不会恢复执行，此预览并非完整对话。",
                )}
              </p>
              {preview.text && <pre>{preview.text}</pre>}
              {preview.tools.length > 0 && (
                <ul>
                  {preview.tools.map((tool) => (
                    <li key={tool.id}>
                      {tool.name} ·{" "}
                      {states[tool.status === "completed" ? "succeeded" : tool.status]}
                    </li>
                  ))}
                </ul>
              )}
              {preview.errorCount > 0 && (
                <p>
                  {label("Recorded execution errors", "已记录的执行错误")}: {preview.errorCount}
                </p>
              )}
              {(preview.truncated ||
                preview.ignoredCount > lifecycleEvents.length ||
                preview.outcome === "unknown") && (
                <p>
                  {label(
                    "Some content is omitted or the recorded attempt is incomplete.",
                    "部分内容已省略，或此次尝试的记录尚不完整。",
                  )}
                </p>
              )}
            </div>
          )}
          {lifecycleEvents.length > 100 && (
            <p>
              {label("Showing the latest 100 recorded changes.", "显示最近 100 条已记录的变化。")}
            </p>
          )}
          <ol>
            {lifecycleEvents.slice(-100).map((event) => {
              const date = new Date(event.created_ms);
              return (
                <li key={event.cursor.sequence}>
                  <span>{eventLabel(event)}</span>
                  <time>
                    {Number.isNaN(date.getTime())
                      ? "—"
                      : date.toLocaleString(zh ? "zh-CN" : "en-US")}
                  </time>
                </li>
              );
            })}
          </ol>
        </>
      )}
    </section>
  );
}

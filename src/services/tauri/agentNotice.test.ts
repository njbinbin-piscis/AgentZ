import type { TFunction } from "i18next";
import { describe, expect, it } from "vitest";
import { agentNoticeText, toastDurationMs } from "./agentNotice";

const t = ((key: string, options?: Record<string, unknown>) =>
  options ? `${key}:${JSON.stringify(options)}` : key) as unknown as TFunction;

describe("agentNoticeText", () => {
  it("localizes the static-overhead notice with its percentage", () => {
    expect(
      agentNoticeText(t, {
        message: "系统提示词与工具定义已占上下文预算的 63%…",
        code: "context_static_overhead",
        details: { percent: 63, static_overhead_tokens: 63000, total_input_budget: 100000 },
      }),
    ).toBe('chat.noticeStaticOverhead:{"percent":"63"}');
  });

  it("localizes the read-loop pause, the loop halt and the model fallback", () => {
    expect(agentNoticeText(t, { message: "检测到重复读取…", code: "read_loop_paused" })).toBe(
      "chat.noticeReadLoopPaused",
    );
    expect(
      agentNoticeText(t, {
        message: "同样的工具调用在提醒和拒绝之后仍被重复，本轮已结束。",
        code: "loop_halted",
        details: { hits: ["Tool 'shell' was refused again"] },
      }),
    ).toBe("chat.noticeLoopHalted");
    expect(
      agentNoticeText(t, {
        message: "主模型「a」当前不可用…",
        code: "model_fallback",
        details: { from: "model-a", to: "model-b" },
      }),
    ).toBe('chat.noticeModelFallback:{"from":"model-a","to":"model-b"}');
  });

  it("falls back to the kernel message for unknown codes and incomplete details", () => {
    expect(agentNoticeText(t, { message: " 工具调用参数不完整 ", code: "tool_args_invalid" })).toBe(
      "工具调用参数不完整",
    );
    expect(agentNoticeText(t, { message: "plain" })).toBe("plain");
    expect(
      agentNoticeText(t, { message: "kernel text", code: "context_static_overhead", details: null }),
    ).toBe("kernel text");
    expect(
      agentNoticeText(t, { message: "kernel text", code: "model_fallback", details: { from: "a" } }),
    ).toBe("kernel text");
  });
});

describe("toastDurationMs", () => {
  it("keeps short messages for the minimum time", () => {
    expect(toastDurationMs("已排队")).toBe(4500);
    expect(toastDurationMs("Queued")).toBe(4500);
  });

  it("keeps a long sentence up longer, within a cap", () => {
    const sentence =
      "系统提示词与工具定义已占上下文预算的 63%，留给对话的空间不足，压缩无法缓解。请停用部分 MCP 工具或技能后重试。";
    expect(toastDurationMs(sentence)).toBeGreaterThan(6000);
    expect(toastDurationMs(sentence)).toBeLessThanOrEqual(12000);
    expect(toastDurationMs("x".repeat(2000))).toBe(12000);
  });
});

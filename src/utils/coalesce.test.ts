import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { coalesce } from "./coalesce";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("coalesce", () => {
  it("runs the first call immediately", () => {
    const fn = vi.fn();
    const c = coalesce(fn, 1000);
    c.call();
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it("merges calls inside the gap into one trailing run at the end of the gap", () => {
    const fn = vi.fn();
    const c = coalesce(fn, 1000);
    c.call();
    c.call();
    c.call();
    expect(fn).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(999);
    expect(fn).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(1);
    expect(fn).toHaveBeenCalledTimes(2);
  });

  it("runs immediately again once the gap has passed", () => {
    const fn = vi.fn();
    const c = coalesce(fn, 1000);
    c.call();
    vi.advanceTimersByTime(1000);
    c.call();
    expect(fn).toHaveBeenCalledTimes(2);
  });

  it("cancel drops a pending trailing run", () => {
    const fn = vi.fn();
    const c = coalesce(fn, 1000);
    c.call();
    c.call();
    c.cancel();
    vi.advanceTimersByTime(5000);
    expect(fn).toHaveBeenCalledTimes(1);
  });
});

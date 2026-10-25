import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
import DelegationSection from "./DelegationSection";

const getMock = vi.fn();
const setMock = vi.fn();
const metricsMock = vi.fn();

vi.mock("../../../services/tauri/delegation", () => ({
  getDelegation: (...a: unknown[]) => getMock(...a),
  setDelegation: (...a: unknown[]) => setMock(...a),
  getDelegationMetrics: (...a: unknown[]) => metricsMock(...a),
}));

const hook = {
  t: (key: string, opts?: Record<string, unknown>) =>
    opts ? `${key}:${JSON.stringify(opts)}` : key,
};
vi.mock("react-i18next", () => ({ useTranslation: () => hook }));

describe("DelegationSection", () => {
  let root: ReactTestRenderer | null = null;
  afterEach(async () => {
    if (root) await act(async () => root?.unmount());
    root = null;
    vi.clearAllMocks();
  });

  it("loads config, shows metrics and saves a mode change", async () => {
    getMock.mockResolvedValue({ mode: "auto", max_concurrent: 4 });
    metricsMock.mockResolvedValue({
      calls: 3,
      failures: 1,
      sub_input_tokens: 100,
      sub_output_tokens: 20,
      sub_tool_calls: 7,
      model_fallbacks: 2,
    });
    setMock.mockImplementation(async (c: unknown) => c);
    await act(async () => {
      root = create(<DelegationSection />);
    });
    root!.root.findByProps({ "data-testid": "delegation-metrics" });
    const rendered = JSON.stringify(root!.toJSON());
    expect(rendered).toContain("\\\"calls\\\":3");
    expect(rendered).toContain("\\\"fallbacks\\\":2");

    const select = root!.root.findByProps({ "data-testid": "delegation-mode" });
    await act(async () => {
      select.props.onChange({ target: { value: "off" } });
    });
    expect(setMock).toHaveBeenCalledWith({ mode: "off", max_concurrent: 4 });
    expect(root!.root.findByProps({ "data-testid": "delegation-mode" }).props.value).toBe("off");
  });

  it("clamps max concurrency into 1..16", async () => {
    getMock.mockResolvedValue({ mode: "auto", max_concurrent: 4 });
    metricsMock.mockResolvedValue({
      calls: 0, failures: 0, sub_input_tokens: 0, sub_output_tokens: 0, sub_tool_calls: 0, model_fallbacks: 0,
    });
    setMock.mockImplementation(async (c: unknown) => c);
    await act(async () => {
      root = create(<DelegationSection />);
    });
    const input = root!.root.findByProps({ "data-testid": "delegation-max" });
    await act(async () => {
      input.props.onChange({ target: { value: "99" } });
    });
    expect(setMock).toHaveBeenLastCalledWith({ mode: "auto", max_concurrent: 16 });
  });
});

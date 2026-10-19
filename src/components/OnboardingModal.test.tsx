import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
import OnboardingModal from "./OnboardingModal";

const tFn = (key: string) => key;
const translationHook = { t: tFn };
vi.mock("react-i18next", () => ({
  useTranslation: () => translationHook,
}));

describe("OnboardingModal 3-step first-run wizard (R10-14)", () => {
  let root: ReactTestRenderer | null = null;

  afterEach(async () => {
    if (root) {
      await act(async () => {
        root?.unmount();
      });
      root = null;
    }
  });

  it("walks through steps 1 -> 2 -> 3, allows mode selection, and calls onFinish", async () => {
    const onOpenSettings = vi.fn();
    const onSelectMode = vi.fn();
    const onFinish = vi.fn();
    const onDismiss = vi.fn();

    await act(async () => {
      root = create(
        <OnboardingModal
          open={true}
          hasConfiguredModel={false}
          currentMode="workz"
          onOpenSettings={onOpenSettings}
          onSelectMode={onSelectMode}
          onFinish={onFinish}
          onDismiss={onDismiss}
        />,
      );
    });

    expect(root!.root.findByProps({ "data-testid": "onboarding-step-1" })).toBeTruthy();

    const cfgBtn = root!.root.findByProps({ "data-testid": "onboarding-open-settings-btn" });
    await act(async () => {
      cfgBtn.props.onClick();
    });
    expect(onOpenSettings).toHaveBeenCalledTimes(1);

    const next1 = root!.root.findByProps({ "data-testid": "onboarding-next-1" });
    await act(async () => {
      next1.props.onClick();
    });
    expect(root!.root.findByProps({ "data-testid": "onboarding-step-2" })).toBeTruthy();

    const codezCard = root!.root.findByProps({ "data-testid": "onboarding-mode-codez" });
    await act(async () => {
      codezCard.props.onClick();
    });
    expect(onSelectMode).toHaveBeenCalledWith("codez");

    const next2 = root!.root.findByProps({ "data-testid": "onboarding-next-2" });
    await act(async () => {
      next2.props.onClick();
    });
    expect(root!.root.findByProps({ "data-testid": "onboarding-step-3" })).toBeTruthy();

    const finishBtn = root!.root.findByProps({ "data-testid": "onboarding-finish-btn" });
    await act(async () => {
      finishBtn.props.onClick();
    });
    expect(onFinish).toHaveBeenCalledTimes(1);
  });
});

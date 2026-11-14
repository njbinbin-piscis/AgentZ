import { useEffect, useRef, type RefObject } from "react";

const NEAR_BOTTOM_PX = 48;

/**
 * Keeps a scroll container pinned to its newest content while the user has not
 * scrolled away. Content growth is detected with a MutationObserver, so rows
 * that grow outside React state changes (tool-call cards, streamed output,
 * expanding blocks) still keep the bottom in view. Growth-induced scroll
 * events are not mistaken for the user scrolling up.
 *
 * `resetKey` identifies the conversation currently shown (session / task /
 * pool / bot id). When it changes the follow state re-arms and the container
 * pins to the bottom, so opening or switching a conversation always lands on
 * the newest message — never on a stale scroll position or the first row.
 */
export function useStickToBottom(
  scrollRef: RefObject<HTMLElement | null>,
  resetKey?: string | null,
) {
  const stickRef = useRef(true);
  const attachedRef = useRef<HTMLElement | null>(null);
  const cleanupRef = useRef<(() => void) | null>(null);
  const pinRef = useRef<(() => void) | null>(null);
  const lastKeyRef = useRef(resetKey);

  useEffect(() => {
    const el = scrollRef.current;
    if (el === attachedRef.current) return;
    cleanupRef.current?.();
    cleanupRef.current = null;
    attachedRef.current = el;
    pinRef.current = null;
    if (!el) return;

    let lastHeight = el.scrollHeight;
    let raf = 0;

    const pin = () => {
      raf = 0;
      el.scrollTop = el.scrollHeight;
      lastHeight = el.scrollHeight;
    };
    const schedulePin = () => {
      if (!stickRef.current || raf) return;
      raf = requestAnimationFrame(pin);
    };
    pinRef.current = schedulePin;

    const onScroll = () => {
      const gap = el.scrollHeight - el.scrollTop - el.clientHeight;
      const grew = el.scrollHeight > lastHeight;
      lastHeight = el.scrollHeight;
      if (gap <= Math.max(NEAR_BOTTOM_PX, el.clientHeight * 0.1)) {
        stickRef.current = true;
      } else if (!grew) {
        stickRef.current = false;
      } else {
        schedulePin();
      }
    };

    el.addEventListener("scroll", onScroll, { passive: true });
    const mo = new MutationObserver(schedulePin);
    mo.observe(el, { childList: true, subtree: true, characterData: true });
    schedulePin();

    cleanupRef.current = () => {
      el.removeEventListener("scroll", onScroll);
      mo.disconnect();
      if (raf) cancelAnimationFrame(raf);
    };
  });

  // Conversation switch: re-arm follow and pin once the new content commits.
  // Runs after the attach effect above so a freshly mounted container is
  // already wired; the MutationObserver keeps the pin honest while markdown /
  // images finish laying out.
  useEffect(() => {
    if (resetKey === lastKeyRef.current) return;
    lastKeyRef.current = resetKey;
    stickRef.current = true;
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
    pinRef.current?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- scrollRef is stable
  }, [resetKey]);

  useEffect(
    () => () => {
      cleanupRef.current?.();
      cleanupRef.current = null;
      attachedRef.current = null;
      pinRef.current = null;
    },
    [],
  );

  return {
    stickRef,
    /** Force-pin on the next frames (e.g. after switching sessions). */
    pinToBottom: () => {
      stickRef.current = true;
      requestAnimationFrame(() =>
        requestAnimationFrame(() => {
          const el = scrollRef.current;
          if (el) el.scrollTop = el.scrollHeight;
        }),
      );
    },
  };
}

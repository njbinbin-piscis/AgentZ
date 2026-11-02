/** Fired after settings are saved so open panels reload LLM / app config. */
export const SETTINGS_REFRESH_EVENT = "agentz-settings-refresh";

/**
 * `silent` refreshes (window regained focus, polling) only reload data; they
 * must not announce "settings saved", since nothing was saved.
 */
export function notifySettingsRefresh(opts?: { silent?: boolean }): void {
  window.dispatchEvent(new CustomEvent(SETTINGS_REFRESH_EVENT, { detail: { silent: opts?.silent === true } }));
}

export function onSettingsRefresh(handler: (info: { silent: boolean }) => void): () => void {
  const listener = (e: Event) => {
    const silent = (e as CustomEvent<{ silent?: boolean } | undefined>).detail?.silent === true;
    handler({ silent });
  };
  window.addEventListener(SETTINGS_REFRESH_EVENT, listener);
  return () => window.removeEventListener(SETTINGS_REFRESH_EVENT, listener);
}

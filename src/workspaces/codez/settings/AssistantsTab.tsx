import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import DropdownSelect from "../../../components/DropdownSelect";
import {
  connectGatewayChannels,
  disconnectGatewayChannels,
  getImSettings,
  listGatewayChannels,
  pollWechatLogin,
  saveImSettings,
  startWechatLogin,
  type ChannelInfo,
  type ChannelStatus,
  type ImSettings,
} from "../../../services/tauri/gateway";

const EMPTY: ImSettings = {
  feishu_app_id: "",
  feishu_app_secret: "",
  feishu_domain: "feishu",
  feishu_enabled: false,
  wecom_bot_id: "",
  wecom_bot_secret: "",
  wecom_enabled: false,
  dingtalk_app_key: "",
  dingtalk_app_secret: "",
  dingtalk_robot_code: "",
  dingtalk_enabled: false,
  telegram_bot_token: "",
  telegram_enabled: false,
  slack_webhook_url: "",
  slack_enabled: false,
  discord_webhook_url: "",
  discord_enabled: false,
  teams_webhook_url: "",
  teams_enabled: false,
  matrix_homeserver: "",
  matrix_access_token: "",
  matrix_room_id: "",
  matrix_enabled: false,
  webhook_outbound_url: "",
  webhook_auth_token: "",
  webhook_enabled: false,
  wechat_enabled: false,
  wechat_gateway_port: 18788,
  wechat_bot_id: "",
  im_message_mode: "queue",
};

function statusLabel(
  t: (key: string, opts?: Record<string, unknown>) => string,
  s: ChannelStatus,
): { text: string; color: string } {
  if (s === "Connected") return { text: t("assistantPanel.statusConnected"), color: "#3fb950" };
  if (s === "Connecting") return { text: t("assistantPanel.statusConnecting"), color: "#d29922" };
  if (s === "Disconnected") return { text: t("assistantPanel.statusDisconnected"), color: "#8b949e" };
  if (typeof s === "object" && "Error" in s)
    return { text: t("assistantPanel.imErrorPrefix", { msg: s.Error }), color: "#f85149" };
  return { text: String(s), color: "#8b949e" };
}

/**
 * Settings tab: IM "assistants" — connect Feishu / WeCom / DingTalk / WeChat etc.
 * so inbound chat messages drive a headless agent turn and the reply is sent back.
 */
export default function AssistantsTab() {
  const { t } = useTranslation();
  const [form, setForm] = useState<ImSettings>(EMPTY);
  const [channels, setChannels] = useState<ChannelInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  // WeChat QR login state.
  const [qr, setQr] = useState<string | null>(null);
  const [qrMsg, setQrMsg] = useState<string | null>(null);
  const pollTimer = useRef<number | null>(null);

  const set = <K extends keyof ImSettings>(key: K, value: ImSettings[K]) =>
    setForm((f) => ({ ...f, [key]: value }));

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [s, st] = await Promise.all([getImSettings(), listGatewayChannels()]);
      setForm(s);
      setChannels(st.channels);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    return () => {
      if (pollTimer.current) window.clearInterval(pollTimer.current);
    };
  }, [refresh]);

  const save = useCallback(async () => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const saved = await saveImSettings(form);
      setForm(saved);
      setNotice(t("assistantPanel.imSaved"));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [form, t]);

  const connect = useCallback(async () => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await saveImSettings(form);
      const st = await connectGatewayChannels();
      setChannels(st.channels);
      setNotice(t("assistantPanel.imConnectedNotice"));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [form, t]);

  const disconnect = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      await disconnectGatewayChannels();
      setChannels([]);
      setNotice(t("assistantPanel.imDisconnectedNotice"));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [t]);

  const beginWechat = useCallback(async () => {
    setError(null);
    setQr(null);
    setQrMsg(t("assistantPanel.wechatQrFetching"));
    try {
      const res = await startWechatLogin();
      setQr(res.qr_data_url);
      setQrMsg(t("assistantPanel.wechatQrPrompt"));
      const token = res.qrcode_token;
      if (!token) return;
      if (pollTimer.current) window.clearInterval(pollTimer.current);
      pollTimer.current = window.setInterval(async () => {
        try {
          const st = await pollWechatLogin(token);
          if (st.message === "scaned") setQrMsg(t("assistantPanel.wechatQrScanned"));
          if (st.connected) {
            setQrMsg(t("assistantPanel.wechatQrConnected"));
            setQr(null);
            if (pollTimer.current) window.clearInterval(pollTimer.current);
            await refresh();
          } else if (st.message === "expired") {
            setQrMsg(t("assistantPanel.wechatQrExpired"));
            setQr(null);
            if (pollTimer.current) window.clearInterval(pollTimer.current);
          }
        } catch (e) {
          setQrMsg(t("assistantPanel.wechatQrPollError", { msg: String(e) }));
          if (pollTimer.current) window.clearInterval(pollTimer.current);
        }
      }, 2500);
    } catch (e) {
      setQrMsg(null);
      setError(String(e));
    }
  }, [refresh, t]);

  if (loading) {
    return <div className="agentz-settings-loading">{t("assistantPanel.imLoading")}</div>;
  }

  const statusOf = (name: string) => channels.find((c) => c.name === name)?.status;

  const renderStatus = (name: string) => {
    const s = statusOf(name);
    if (!s) return null;
    const { text, color } = statusLabel(t, s);
    return (
      <span className="agentz-wb-meta" style={{ color }}>
        ● {text}
      </span>
    );
  };

  return (
    <div className="agentz-settings-tabpanel">
      <section className="agentz-settings-section">
        <h3>{t("assistantPanel.imTitle")}</h3>
        <p className="agentz-settings-hint">
          {t("assistantPanel.imHint")}
        </p>
        {error && <div className="agentz-settings-error">{error}</div>}
        {notice && <div className="agentz-settings-hint">{notice}</div>}

        {/* Feishu / Lark */}
        <div className="agentz-wb-row" style={{ flexDirection: "column", alignItems: "stretch", gap: 8 }}>
          <label style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <input
              type="checkbox"
              checked={form.feishu_enabled}
              onChange={(e) => set("feishu_enabled", e.target.checked)}
            />
            <strong>{t("assistantPanel.feishuTitle")}</strong>
            {renderStatus("feishu")}
          </label>
          <input
            placeholder="App ID"
            value={form.feishu_app_id}
            onChange={(e) => set("feishu_app_id", e.target.value)}
          />
          <input
            placeholder="App Secret"
            type="password"
            value={form.feishu_app_secret}
            onChange={(e) => set("feishu_app_secret", e.target.value)}
          />
          <DropdownSelect
            variant="field"
            value={form.feishu_domain}
            options={[
              { id: "feishu", label: t("assistantPanel.feishuDomainDomestic") },
              { id: "lark", label: t("assistantPanel.feishuDomainGlobal") },
            ]}
            onChange={(v) => set("feishu_domain", v)}
          />
        </div>

        {/* WeCom */}
        <div className="agentz-wb-row" style={{ flexDirection: "column", alignItems: "stretch", gap: 8 }}>
          <label style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <input
              type="checkbox"
              checked={form.wecom_enabled}
              onChange={(e) => set("wecom_enabled", e.target.checked)}
            />
            <strong>{t("assistantPanel.wecomTitle")}</strong>
            {renderStatus("wecom")}
          </label>
          <input
            placeholder="Bot ID"
            value={form.wecom_bot_id}
            onChange={(e) => set("wecom_bot_id", e.target.value)}
          />
          <input
            placeholder="Bot Secret"
            type="password"
            value={form.wecom_bot_secret}
            onChange={(e) => set("wecom_bot_secret", e.target.value)}
          />
        </div>

        {/* DingTalk */}
        <div className="agentz-wb-row" style={{ flexDirection: "column", alignItems: "stretch", gap: 8 }}>
          <label style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <input
              type="checkbox"
              checked={form.dingtalk_enabled}
              onChange={(e) => set("dingtalk_enabled", e.target.checked)}
            />
            <strong>{t("assistantPanel.dingtalkTitle")}</strong>
            {renderStatus("dingtalk")}
          </label>
          <input
            placeholder="App Key"
            value={form.dingtalk_app_key}
            onChange={(e) => set("dingtalk_app_key", e.target.value)}
          />
          <input
            placeholder="App Secret"
            type="password"
            value={form.dingtalk_app_secret}
            onChange={(e) => set("dingtalk_app_secret", e.target.value)}
          />
          <input
            placeholder={t("assistantPanel.dingtalkRobotCodePlaceholder")}
            value={form.dingtalk_robot_code}
            onChange={(e) => set("dingtalk_robot_code", e.target.value)}
          />
        </div>

        {/* WeChat */}
        <div className="agentz-wb-row" style={{ flexDirection: "column", alignItems: "stretch", gap: 8 }}>
          <label style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <input
              type="checkbox"
              checked={form.wechat_enabled}
              onChange={(e) => set("wechat_enabled", e.target.checked)}
            />
            <strong>{t("assistantPanel.wechatTitle")}</strong>
            {renderStatus("wechat")}
          </label>
          <div style={{ fontSize: 12, color: "var(--text-dim)" }}>
            {form.wechat_bot_id
              ? t("assistantPanel.wechatBound", { id: form.wechat_bot_id })
              : t("assistantPanel.wechatUnbound")}
          </div>
          <div className="agentz-wb-actions">
            <button type="button" onClick={() => void beginWechat()}>
              {t("assistantPanel.wechatScanLogin")}
            </button>
          </div>
          {qrMsg && <div className="agentz-settings-hint">{qrMsg}</div>}
          {qr && (
            <img
              src={qr}
              alt={t("assistantPanel.wechatQrAlt")}
              style={{ width: 200, height: 200, background: "#fff", padding: 8, borderRadius: 6 }}
            />
          )}
        </div>

        {/* Message handling mode */}
        <div className="agentz-wb-row" style={{ flexDirection: "column", alignItems: "stretch", gap: 8 }}>
          <label style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <strong>{t("assistantPanel.imModeTitle")}</strong>
          </label>
          <DropdownSelect
            variant="field"
            value={form.im_message_mode || "queue"}
            options={[
              { id: "queue", label: t("assistantPanel.imModeQueue") },
              { id: "cancel", label: t("assistantPanel.imModeCancel") },
            ]}
            onChange={(v) => set("im_message_mode", v)}
          />
        </div>

        <div className="agentz-wb-actions" style={{ marginTop: 12 }}>
          <button type="button" onClick={() => void save()} disabled={busy}>
            {busy ? t("assistantPanel.imSaving") : t("assistantPanel.imSave")}
          </button>
          <button type="button" onClick={() => void connect()} disabled={busy}>
            {t("assistantPanel.imSaveAndConnect")}
          </button>
          <button type="button" className="danger" onClick={() => void disconnect()} disabled={busy}>
            {t("assistantPanel.imDisconnectAll")}
          </button>
        </div>
      </section>
    </div>
  );
}

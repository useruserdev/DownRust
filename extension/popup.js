// SPDX-License-Identifier: GPL-3.0-or-later

const api = globalThis.browser ?? globalThis.chrome;

const DEFAULT_PORT = 52151;
const PING_TIMEOUT_MS = 800;
const TOKEN_HEADER = "X-DownRust-Token";
const KIND_LABELS = { hls: "HLS", dash: "DASH" };

const statusLine = document.getElementById("status");
const list = document.getElementById("list");
const empty = document.getElementById("empty");

function setStatus(text) {
  statusLine.textContent = text;
  statusLine.hidden = !text;
}

async function loadSettings() {
  const stored = await api.storage.local.get({ port: DEFAULT_PORT, token: "" });
  return {
    port: Number(stored.port) || DEFAULT_PORT,
    token: String(stored.token || "").trim(),
  };
}

function appUrl(settings, path) {
  return `http://127.0.0.1:${settings.port}${path}`;
}

async function isAppRunning(settings) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), PING_TIMEOUT_MS);
  try {
    const response = await fetch(appUrl(settings, "/v1/ping"), {
      headers: { [TOKEN_HEADER]: settings.token },
      signal: controller.signal,
    });
    return response.ok;
  } catch {
    return false;
  } finally {
    clearTimeout(timer);
  }
}

/** The streams the background script found for one tab. */
async function streamsForTab(tabId) {
  const area = api.storage.session ?? api.storage.local;
  const key = `streams:${tabId}`;
  const stored = await area.get(key);
  return Array.isArray(stored[key]) ? stored[key] : [];
}

async function activeTab() {
  const [tab] = await api.tabs.query({ active: true, currentWindow: true });
  return tab;
}

/** Host and path of an address, shortened so it fits in the popup. */
function shortAddress(url) {
  try {
    const parsed = new URL(url);
    const path = parsed.pathname.length > 48 ? `...${parsed.pathname.slice(-45)}` : parsed.pathname;
    return `${parsed.host}${path}`;
  } catch {
    return url;
  }
}

async function sendStream(stream, button) {
  button.disabled = true;
  try {
    const settings = await loadSettings();
    if (!settings.token) {
      setStatus("Set the token in the DownRust options first.");
      return;
    }
    if (!(await isAppRunning(settings))) {
      setStatus("DownRust is not running.");
      return;
    }
    const cookies = await api.cookies
      .getAll({ url: stream.url })
      .then((items) => items.map((cookie) => `${cookie.name}=${cookie.value}`).join("; "))
      .catch(() => "");
    const body = {
      url: stream.url,
      kind: stream.kind,
      title: stream.title || undefined,
      referrer: stream.pageUrl || undefined,
      cookies: cookies || undefined,
      userAgent: navigator.userAgent,
    };
    const response = await fetch(appUrl(settings, "/v1/downloads"), {
      method: "POST",
      headers: {
        [TOKEN_HEADER]: settings.token,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
    });
    setStatus(response.ok ? "Sent to DownRust." : `DownRust answered HTTP ${response.status}.`);
  } catch (error) {
    setStatus(`Could not reach DownRust: ${error.message}`);
  } finally {
    button.disabled = false;
  }
}

function renderStreams(streams) {
  list.replaceChildren();
  empty.hidden = streams.length > 0;
  for (const stream of streams) {
    const item = document.createElement("li");
    item.className = "item";

    const text = document.createElement("div");
    text.className = "text";
    const title = document.createElement("div");
    title.className = "title";
    title.textContent = stream.title || shortAddress(stream.url);
    const meta = document.createElement("div");
    meta.className = "meta";
    const label = document.createElement("span");
    label.className = "kind";
    label.textContent = KIND_LABELS[stream.kind] || stream.kind.toUpperCase();
    meta.append(label, ` ${shortAddress(stream.url)}`);
    text.append(title, meta);

    const button = document.createElement("button");
    button.type = "button";
    button.className = "download";
    button.textContent = "Download";
    button.addEventListener("click", () => sendStream(stream, button));

    item.append(text, button);
    list.append(item);
  }
}

async function init() {
  const tab = await activeTab();
  if (!tab || tab.id === undefined) {
    renderStreams([]);
    return;
  }
  renderStreams(await streamsForTab(tab.id));
}

init().catch((error) => setStatus(`Could not read the streams: ${error.message}`));

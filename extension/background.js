// SPDX-License-Identifier: GPL-3.0-or-later

const api = globalThis.browser ?? globalThis.chrome;

const DEFAULT_PORT = 52151;
const PING_TIMEOUT_MS = 800;
const TOKEN_HEADER = "X-DownRust-Token";
const MENU_ID = "downrust-link";
const MENU_TITLE = "Download with DownRust";
const GRAB_ALL_ID = "downrust-grab-all";
const GRAB_ALL_TITLE = "Download all links with DownRust";
const GRAB_SELECTED_ID = "downrust-grab-selected";
const GRAB_SELECTED_TITLE = "Download selected links with DownRust";
const NATIVE_HOST = "com.downrust.bridge";
const BATCH_LIMIT = 5000;
const BADGE_COLOR = "#F26A2E";
const MAX_STREAMS_PER_TAB = 20;
const WEB_REQUEST_TYPES = ["xmlhttprequest", "media", "other"];
const HLS_TYPES = [
  "application/vnd.apple.mpegurl",
  "application/x-mpegurl",
  "audio/mpegurl",
];
const DASH_TYPE = "application/dash+xml";

// The service worker can stop at any time, so nothing is kept in globals.
// Settings are read from storage on every event.
async function loadSettings() {
  let stored = await readStoredSettings();
  if (!stored.token) {
    await connectAutomatically();
    stored = await readStoredSettings();
  }
  return {
    enabled: stored.enabled !== false,
    port: Number(stored.port) || DEFAULT_PORT,
    token: String(stored.token || "").trim(),
  };
}

function readStoredSettings() {
  return api.storage.local.get({
    enabled: true,
    port: DEFAULT_PORT,
    token: "",
    manual: false,
  });
}

/**
 * Asks the DownRust desktop app for its bridge settings through native messaging.
 * The port and token become the active connection unless the user set them manually.
 * Failures are silent: the stored manual values stay in use.
 */
async function connectAutomatically() {
  let config = null;
  try {
    config = await api.runtime.sendNativeMessage(NATIVE_HOST, { type: "getConfig" });
  } catch {
    config = null;
  }
  if (!config || config.type !== "config") {
    await api.storage.local.set({ autoState: "not-found", autoVersion: "" });
    return;
  }
  const version = String(config.version || "");
  if (config.enabled === false) {
    await api.storage.local.set({ autoState: "bridge-off", autoVersion: version });
    return;
  }
  const stored = await readStoredSettings();
  const values = { autoState: "connected", autoVersion: version };
  if (!stored.manual) {
    values.port = Number(config.port) || DEFAULT_PORT;
    values.token = String(config.token || "").trim();
  }
  await api.storage.local.set(values);
}

function appUrl(settings, path) {
  return `http://127.0.0.1:${settings.port}${path}`;
}

function isHttpUrl(url) {
  return typeof url === "string" && /^https?:\/\//i.test(url);
}

function baseName(path) {
  return path ? path.split(/[\\/]/).pop() : "";
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

async function cookieHeaderFor(url) {
  const cookies = await api.cookies.getAll({ url });
  return cookies.map((cookie) => `${cookie.name}=${cookie.value}`).join("; ");
}

async function sendToApp(settings, job) {
  const cookies = await cookieHeaderFor(job.url).catch(() => "");
  const body = {
    url: job.url,
    fileName: job.fileName || undefined,
    referrer: job.referrer || undefined,
    cookies: cookies || undefined,
    userAgent: navigator.userAgent,
    fileSize: job.fileSize > 0 ? job.fileSize : undefined,
    mime: job.mime || undefined,
    kind: job.kind || undefined,
    title: job.title || undefined,
  };
  const response = await fetch(appUrl(settings, "/v1/downloads"), {
    method: "POST",
    headers: {
      [TOKEN_HEADER]: settings.token,
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    throw new Error(`DownRust answered HTTP ${response.status}`);
  }
}

async function onDownloadCreated(item) {
  try {
    const settings = await loadSettings();
    if (!settings.enabled || !settings.token) return;
    if (!isHttpUrl(item.url) || item.byExtensionId) return;
    if (!(await isAppRunning(settings))) return;

    await api.downloads.cancel(item.id);
    await api.downloads.erase({ id: item.id });
    try {
      await sendToApp(settings, {
        url: item.url,
        fileName: baseName(item.filename),
        referrer: item.referrer,
        fileSize: item.fileSize,
        mime: item.mime,
      });
    } catch (error) {
      // The browser download is already cancelled: start it again in the browser so the file is not lost.
      // Downloads started by this extension carry byExtensionId and are not intercepted again.
      await api.downloads.download({ url: item.url });
      throw error;
    }
  } catch (error) {
    console.error("DownRust: could not pass the download to the app", error);
  }
}

async function onLinkMenuClick(info) {
  if (info.menuItemId !== MENU_ID || !info.linkUrl) return;
  try {
    const settings = await loadSettings();
    if (!settings.enabled || !settings.token) return;
    if (!isHttpUrl(info.linkUrl)) return;
    if (!(await isAppRunning(settings))) return;

    await sendToApp(settings, {
      url: info.linkUrl,
      referrer: info.pageUrl,
    });
  } catch (error) {
    console.error("DownRust: could not pass the link to the app", error);
  }
}

async function onGrabLinks(info, tab, selectedOnly) {
  try {
    if (!tab || tab.id === undefined || tab.id < 0) return;
    const settings = await loadSettings();
    if (!settings.enabled || !settings.token) return;
    if (!(await isAppRunning(settings))) return;

    const links = await collectLinks(tab.id, selectedOnly);
    if (links.length === 0) return;
    await sendBatch(settings, links, info.pageUrl || tab.url);
  } catch (error) {
    console.error("DownRust: could not pass the links to the app", error);
  }
}

async function collectLinks(tabId, selectedOnly) {
  const results = await api.scripting.executeScript({
    target: { tabId },
    func: collectLinksInPage,
    args: [selectedOnly],
  });
  const first = Array.isArray(results) ? results[0] : null;
  return first && Array.isArray(first.result) ? first.result : [];
}

/**
 * Runs inside the page. It must not use anything from this file, because the browser
 * serializes the function and runs it in the page context.
 */
function collectLinksInPage(selectedOnly) {
  const LIMIT = 5000;
  const TEXT_LIMIT = 200;
  const SCHEMES = ["http:", "https:", "ftp:", "ftps:", "sftp:", "magnet:"];
  let range = null;
  if (selectedOnly) {
    const selection = window.getSelection();
    if (!selection || selection.rangeCount === 0 || selection.isCollapsed) return [];
    range = selection.getRangeAt(0);
  }
  const seen = new Set();
  const links = [];
  const elements = document.querySelectorAll("a[href], img[src], video[src], audio[src], source[src]");
  for (const element of elements) {
    if (links.length >= LIMIT) break;
    if (range && !range.intersectsNode(element)) continue;
    const attribute = element.localName === "a" ? "href" : "src";
    const raw = (element.getAttribute(attribute) || "").trim();
    if (!raw || raw.startsWith("#")) continue;
    let url;
    try {
      url = new URL(raw, document.baseURI);
    } catch {
      continue;
    }
    if (!SCHEMES.includes(url.protocol)) continue;
    const href = url.href;
    if (seen.has(href)) continue;
    seen.add(href);
    const label = (element.textContent || element.getAttribute("alt") || element.getAttribute("title") || "")
      .replace(/\s+/g, " ")
      .trim();
    links.push({ url: href, text: label.slice(0, TEXT_LIMIT) });
  }
  return links;
}

async function sendBatch(settings, links, referrer) {
  const response = await fetch(appUrl(settings, "/v1/batch"), {
    method: "POST",
    headers: {
      [TOKEN_HEADER]: settings.token,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      links: links.slice(0, BATCH_LIMIT),
      referrer: referrer || undefined,
      userAgent: navigator.userAgent,
    }),
  });
  if (!response.ok) {
    throw new Error(`DownRust answered HTTP ${response.status}`);
  }
}

function createContextMenu() {
  Promise.resolve(api.contextMenus.removeAll()).then(() => {
    api.contextMenus.create({
      id: MENU_ID,
      title: MENU_TITLE,
      contexts: ["link"],
    });
    api.contextMenus.create({
      id: GRAB_ALL_ID,
      title: GRAB_ALL_TITLE,
      contexts: ["page"],
    });
    api.contextMenus.create({
      id: GRAB_SELECTED_ID,
      title: GRAB_SELECTED_TITLE,
      contexts: ["selection"],
    });
  });
}

// ---- Stream detection -------------------------------------------------------

/** `storage.session` where it exists. Firefox versions without it fall back to `storage.local`. */
function streamArea() {
  return api.storage.session ?? api.storage.local;
}

function streamKey(tabId) {
  return `streams:${tabId}`;
}

/** The kind of stream named by a URL path: `.m3u8` is HLS and `.mpd` is DASH. */
function kindFromUrl(url) {
  const path = String(url || "").split(/[?#]/)[0];
  if (/\.m3u8$/i.test(path)) return "hls";
  if (/\.mpd$/i.test(path)) return "dash";
  return null;
}

/** The kind of stream named by a Content-Type value. Parameters after `;` are ignored. */
function kindFromContentType(value) {
  const type = String(value || "").split(";")[0].trim().toLowerCase();
  if (HLS_TYPES.includes(type)) return "hls";
  if (type === DASH_TYPE) return "dash";
  return null;
}

function contentTypeOf(details) {
  const header = (details.responseHeaders || []).find(
    (item) => item.name.toLowerCase() === "content-type",
  );
  return header ? header.value : "";
}

// Writes to the list of one tab run one after another, so events do not overwrite each other.
let writeQueue = Promise.resolve();

function serialized(task) {
  const next = writeQueue.then(task).catch((error) => {
    console.error("DownRust: stream list update failed", error);
  });
  writeQueue = next;
  return next;
}

async function readStreams(tabId) {
  const stored = await streamArea().get(streamKey(tabId));
  return Array.isArray(stored[streamKey(tabId)]) ? stored[streamKey(tabId)] : [];
}

async function updateBadge(tabId, count) {
  await api.action.setBadgeBackgroundColor({ tabId, color: BADGE_COLOR });
  await api.action.setBadgeText({ tabId, text: count > 0 ? String(count) : "" });
}

function rememberStream(tabId, url, kind, pageUrl) {
  return serialized(async () => {
    const tab = await api.tabs.get(tabId).catch(() => null);
    const streams = await readStreams(tabId);
    if (streams.some((item) => item.url === url)) return;
    const title = (tab && tab.title) || baseName(new URL(url).pathname) || url;
    const entry = {
      url,
      kind,
      pageUrl: pageUrl || (tab && tab.url) || "",
      title,
      at: Date.now(),
    };
    const next = [entry, ...streams].slice(0, MAX_STREAMS_PER_TAB);
    await streamArea().set({ [streamKey(tabId)]: next });
    await updateBadge(tabId, next.length);
  });
}

function clearStreams(tabId) {
  return serialized(async () => {
    await streamArea().remove(streamKey(tabId));
    await updateBadge(tabId, 0);
  });
}

function onBeforeRequest(details) {
  if (details.tabId < 0) return;
  const kind = kindFromUrl(details.url);
  if (kind) {
    rememberStream(details.tabId, details.url, kind, details.documentUrl || details.originUrl);
  }
}

function onHeadersReceived(details) {
  if (details.tabId < 0) return;
  const kind = kindFromContentType(contentTypeOf(details));
  if (kind) {
    rememberStream(details.tabId, details.url, kind, details.documentUrl || details.originUrl);
  }
}

api.runtime.onInstalled.addListener(() => {
  createContextMenu();
  connectAutomatically();
});
api.runtime.onStartup.addListener(() => {
  connectAutomatically();
});
api.downloads.onCreated.addListener((item) => {
  onDownloadCreated(item);
});
api.contextMenus.onClicked.addListener((info, tab) => {
  if (info.menuItemId === MENU_ID) {
    onLinkMenuClick(info);
  } else if (info.menuItemId === GRAB_ALL_ID) {
    onGrabLinks(info, tab, false);
  } else if (info.menuItemId === GRAB_SELECTED_ID) {
    onGrabLinks(info, tab, true);
  }
});
api.webRequest.onBeforeRequest.addListener(onBeforeRequest, {
  urls: ["<all_urls>"],
  types: WEB_REQUEST_TYPES,
});
api.webRequest.onHeadersReceived.addListener(
  onHeadersReceived,
  { urls: ["<all_urls>"], types: WEB_REQUEST_TYPES },
  ["responseHeaders"],
);
api.tabs.onUpdated.addListener((tabId, changeInfo) => {
  if (changeInfo.url && changeInfo.status === "loading") {
    clearStreams(tabId);
  }
});
api.tabs.onRemoved.addListener((tabId) => {
  clearStreams(tabId);
});

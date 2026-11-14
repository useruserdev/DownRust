// SPDX-License-Identifier: GPL-3.0-or-later

const api = globalThis.browser ?? globalThis.chrome;
const isFirefox = typeof globalThis.browser !== "undefined";

const DEFAULT_PORT = 52151;
const TEST_TIMEOUT_MS = 2000;
const TOKEN_HEADER = "X-DownRust-Token";

const enabledInput = document.getElementById("enabled");
const portInput = document.getElementById("port");
const tokenInput = document.getElementById("token");
const saveButton = document.getElementById("save");
const testButton = document.getElementById("test");
const grantButton = document.getElementById("grant");
const firefoxSection = document.getElementById("firefox-only");
const statusLine = document.getElementById("status");
const testResult = document.getElementById("test-result");

function showStatus(message) {
  statusLine.textContent = message;
}

function readForm() {
  return {
    enabled: enabledInput.checked,
    port: Number.parseInt(portInput.value, 10),
    token: tokenInput.value.trim(),
  };
}

function isValidPort(port) {
  return Number.isInteger(port) && port >= 1 && port <= 65535;
}

async function restore() {
  const stored = await api.storage.local.get({
    enabled: true,
    port: DEFAULT_PORT,
    token: "",
  });
  enabledInput.checked = stored.enabled !== false;
  portInput.value = stored.port;
  tokenInput.value = stored.token;
}

saveButton.addEventListener("click", async () => {
  const form = readForm();
  if (!isValidPort(form.port)) {
    showStatus("Port must be a whole number from 1 to 65535.");
    return;
  }
  try {
    await api.storage.local.set(form);
    showStatus("Saved.");
  } catch (error) {
    showStatus(`Could not save: ${error.message}`);
  }
});

testButton.addEventListener("click", async () => {
  const form = readForm();
  testResult.textContent = "";
  if (!isValidPort(form.port)) {
    testResult.textContent = "Port must be a whole number from 1 to 65535.";
    return;
  }
  if (!form.token) {
    testResult.textContent = "Enter the token first.";
    return;
  }

  testResult.textContent = "Testing...";
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), TEST_TIMEOUT_MS);
  try {
    const response = await fetch(`http://127.0.0.1:${form.port}/v1/ping`, {
      headers: { [TOKEN_HEADER]: form.token },
      signal: controller.signal,
    });
    if (response.status === 401) {
      testResult.textContent = "DownRust rejected the token.";
    } else if (!response.ok) {
      testResult.textContent = `DownRust answered HTTP ${response.status}.`;
    } else {
      const info = await response.json().catch(() => ({}));
      const name = info.name ?? "DownRust";
      const version = info.version ? ` ${info.version}` : "";
      testResult.textContent = `Connected to ${name}${version}.`;
    }
  } catch {
    testResult.textContent =
      `No answer on 127.0.0.1:${form.port}. Check that DownRust is running and the browser bridge is enabled.`;
  } finally {
    clearTimeout(timer);
  }
});

if (isFirefox) {
  firefoxSection.hidden = false;
  grantButton.addEventListener("click", async () => {
    try {
      const granted = await api.permissions.request({ origins: ["<all_urls>"] });
      showStatus(granted ? "Access granted." : "Access was not granted.");
    } catch (error) {
      showStatus(`Could not request access: ${error.message}`);
    }
  });
}

restore().catch((error) => {
  showStatus(`Could not load settings: ${error.message}`);
});

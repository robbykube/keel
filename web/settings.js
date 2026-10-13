const $ = (id) => document.getElementById(id);

let connectTimer = 0;
let lastConnect = null;
let updateProgressTimer = 0;
let lastUpdateProgress = null;

function updateButton(label, action) {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = label;
  button.addEventListener("click", action);
  return button;
}

function setConnectStatus(text) {
  $("github-connect-status").textContent = text || "";
}

function showConnect(status) {
  if (!status) return;
  lastConnect = status;
  const connected = status.state === "connected" || status.github_token_set;
  const pending = status.state === "pending" && status.user_code;
  $("github-connect-button").hidden = connected || pending;
  $("github-disconnect-button").hidden = !connected || pending;
  $("github-device").hidden = !pending;
  if (pending) {
    $("github-user-code").textContent = status.user_code;
    const link = $("github-device-link");
    link.href = status.verification_uri || "https://github.com/login/device";
    $("github-device-message").textContent = status.message || "";
    scheduleConnectPoll();
  } else {
    stopConnectPoll();
  }
  const params = new URLSearchParams(window.location.search);
  if (params.get("github") === "ready") {
    setConnectStatus("GitHub allowed this computer. Press Connect GitHub again for the code. If GitHub shows Device Flow, tick it on the Keel app first.");
    return;
  }
  if (params.get("github") === "failed") {
    setConnectStatus("GitHub did not finish allowing this computer. Press Connect GitHub to try again.");
    return;
  }
  if (status.state === "expired" || status.state === "denied" || status.state === "error") {
    setConnectStatus(status.message || "");
  } else if (connected && !pending) {
    setConnectStatus(status.message || "GitHub is connected on this computer.");
  } else if (!pending) {
    setConnectStatus(status.message || "");
  } else {
    setConnectStatus("");
  }
}

function allowExistingRepo() {
  const origin = window.location.origin;
  const manifest = {
    name: "Keel",
    url: "https://github.com/robbykube/keel",
    redirect_url: `${origin}/api/settings/github-app`,
    callback_urls: [`${origin}/api/settings/github-app`],
    description: "Lets this Keel computer see private releases from robbykube/keel.",
    public: false,
    hook_attributes: { url: "https://github.com/robbykube/keel", active: false },
    default_permissions: { contents: "read", metadata: "read" },
  };
  const form = document.createElement("form");
  form.method = "post";
  form.action = "https://github.com/settings/apps/new";
  const input = document.createElement("input");
  input.type = "hidden";
  input.name = "manifest";
  input.value = JSON.stringify(manifest);
  form.appendChild(input);
  document.body.appendChild(form);
  form.submit();
}

function renderUpdate(update) {
  const box = $("updates");
  if (!box || !update) return;
  box.replaceChildren();
  const text = document.createElement("span");
  text.textContent = update.message || "Keel could not check for an update.";
  box.appendChild(text);
  if (update.release_url) {
    const link = document.createElement("a");
    link.href = update.release_url;
    link.rel = "noreferrer";
    link.textContent = " Release notes";
    box.appendChild(link);
  }
  const updateRunning = lastUpdateProgress && ["queued", "running"].includes(lastUpdateProgress.state);
  if (update.can_apply && !updateRunning) {
    box.appendChild(updateButton("Install update", () => postUpdate("/api/updates/install", "INSTALL UPDATE", "Install this Keel update? If the new version does not start, the previous version is restored.")));
  }
  if (update.can_restore) {
    box.appendChild(updateButton("Restore the previous version", () => postUpdate("/api/updates/restore", "RESTORE PREVIOUS", "Put the previous Keel back? This machine will restart with the saved version.")));
  }
}

function renderUpdateProgress(progress) {
  if (!progress) return;
  lastUpdateProgress = progress;
  const panel = $("update-progress");
  const message = $("update-progress-message");
  const bar = $("update-progress-bar");
  if (!panel || !message || !bar) return;
  const percent = Math.max(0, Math.min(100, Number(progress.percent) || 0));
  panel.hidden = progress.state === "idle";
  message.textContent = progress.message || "";
  bar.style.width = `${percent}%`;
  bar.parentElement.setAttribute("aria-valuenow", String(percent));
  if (progress.state === "error") message.classList.add("install-error");
  else message.classList.remove("install-error");
  if (progress.state === "queued" || progress.state === "running") scheduleUpdateProgressPoll();
  else stopUpdateProgressPoll();
}

function scheduleUpdateProgressPoll() {
  if (updateProgressTimer) return;
  updateProgressTimer = window.setTimeout(() => {
    updateProgressTimer = 0;
    loadUpdateProgress();
  }, 1200);
}

function stopUpdateProgressPoll() {
  if (updateProgressTimer) {
    window.clearTimeout(updateProgressTimer);
    updateProgressTimer = 0;
  }
}

async function loadUpdateProgress() {
  try {
    const res = await fetch("/api/updates/progress", { credentials: "same-origin" });
    if (!res.ok) return;
    renderUpdateProgress(await res.json());
    if (lastUpdateProgress && ["queued", "running"].includes(lastUpdateProgress.state)) return;
    if (lastUpdateProgress && ["done", "error"].includes(lastUpdateProgress.state)) await loadUpdates();
  } catch (_) {
    scheduleUpdateProgressPoll();
  }
}

async function loadUpdates() {
  try {
    const res = await fetch("/api/updates", { credentials: "same-origin" });
    const body = await res.json();
    const update = body.data ?? body;
    renderUpdate(update);
  } catch (err) {
    $("updates").textContent = String(err.message || err);
  }
}

async function postJson(path, payload) {
  const res = await fetch(path, {
    method: "POST",
    credentials: "same-origin",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(payload),
  });
  const text = await res.text();
  let body = text;
  try { body = JSON.parse(text); } catch { /* Sign-in answers with plain text. */ }
  return { res, body, message: body.message || text };
}

async function postUpdate(path, confirmation, question) {
  if (!window.confirm(question)) return;
  const box = $("updates");
  box.textContent = "Working…";
  try {
    const { res, message } = await postJson(path, { confirmation });
    box.textContent = message;
    await loadUpdateProgress();
    if (res.status === 401 || res.status === 403) {
      box.innerHTML = `${message} <a href="/login.html">Sign in, then try again.</a>`;
    }
  } catch (error) {
    box.textContent = String(error);
  }
}

async function refreshConnect() {
  try {
    const res = await fetch("/api/settings/github-connect", { credentials: "same-origin" });
    const text = await res.text();
    let body = text;
    try { body = JSON.parse(text); } catch { /* Sign-in answers with plain text. */ }
    if (res.status === 401 || res.status === 403) {
      setConnectStatus(`${body.message || text} Sign in, then try again.`);
      return;
    }
    if (!res.ok) {
      setConnectStatus(body.message || text);
      return;
    }
    showConnect(body);
    if (body.state === "connected") await loadUpdates();
  } catch (error) {
    setConnectStatus(String(error.message || error));
  }
}

function scheduleConnectPoll() {
  if (connectTimer) return;
  connectTimer = window.setTimeout(() => {
    connectTimer = 0;
    refreshConnect();
  }, 2000);
}

function stopConnectPoll() {
  if (connectTimer) {
    window.clearTimeout(connectTimer);
    connectTimer = 0;
  }
}

$("github-connect-button").addEventListener("click", async () => {
  if (lastConnect && lastConnect.needs_setup) {
    setConnectStatus("Opening GitHub to allow this computer…");
    allowExistingRepo();
    return;
  }
  $("github-connect-button").disabled = true;
  setConnectStatus("Starting GitHub sign-in…");
  try {
    const { res, body, message } = await postJson("/api/settings/github-connect", {});
    if (res.status === 401 || res.status === 403) {
      setConnectStatus(`${message} Sign in, then try again.`);
      return;
    }
    if (!res.ok) {
      setConnectStatus(message);
      $("github-manual").open = true;
      return;
    }
    showConnect(body);
  } catch (error) {
    setConnectStatus(String(error.message || error));
  } finally {
    $("github-connect-button").disabled = false;
  }
});

$("github-cancel").addEventListener("click", async () => {
  try {
    await postJson("/api/settings/github-connect/cancel", {});
  } catch (_) { /* Keep the page usable. */ }
  stopConnectPoll();
  $("github-device").hidden = true;
  $("github-connect-button").hidden = false;
  setConnectStatus("GitHub sign-in cancelled.");
});

$("github-disconnect-button").addEventListener("click", async () => {
  setConnectStatus("Disconnecting…");
  try {
    const { res, message } = await postJson("/api/settings/github-token", { token: "" });
    if (!res.ok) {
      setConnectStatus(message);
      return;
    }
    setConnectStatus(message);
    $("github-disconnect-button").hidden = true;
    await refreshConnect();
    await loadUpdates();
  } catch (error) {
    setConnectStatus(String(error.message || error));
  }
});

$("token-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  setConnectStatus("Saving…");
  try {
    const { res, message } = await postJson("/api/settings/github-token", { token: $("github-token").value });
    if (res.status === 401 || res.status === 403) {
      setConnectStatus(`${message} Sign in, then try again.`);
      return;
    }
    if (!res.ok) {
      setConnectStatus(message);
      return;
    }
    $("github-token").value = "";
    setConnectStatus(message);
    await loadUpdates();
    await refreshConnect();
  } catch (error) {
    setConnectStatus(String(error.message || error));
  }
});

$("client-id-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  setConnectStatus("Saving…");
  try {
    const { res, message } = await postJson("/api/settings/github-client-id", { client_id: $("github-client-id").value });
    if (res.status === 401 || res.status === 403) {
      setConnectStatus(`${message} Sign in, then try again.`);
      return;
    }
    if (!res.ok) {
      setConnectStatus(message);
      return;
    }
    $("github-client-id").value = "";
    setConnectStatus(message);
    await refreshConnect();
  } catch (error) {
    setConnectStatus(String(error.message || error));
  }
});

loadUpdates();
loadUpdateProgress();
refreshConnect();

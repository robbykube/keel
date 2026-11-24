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
  // Until GitHub is connected, the box that connects it stays open.
  if (!connected || pending) $("github-why").open = true;
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

let updateHold = false;

function renderUpdateProgress(progress) {
  if (!progress) return;
  if (updateHold && (progress.state === "idle" || progress.state === "done")) return;
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
  const starting = path.endsWith("/restore")
    ? "Restoring the previous Keel."
    : "Downloading the Keel update.";
  updateHold = true;
  renderUpdateProgress({ state: "running", percent: 1, message: starting });
  try {
    const { res, message } = await postJson(path, { confirmation });
    updateHold = false;
    if (!res.ok) {
      stopUpdateProgressPoll();
      renderUpdateProgress({ state: "error", percent: 0, message: message || "Keel could not continue." });
      if (res.status === 401 || res.status === 403) {
        $("update-progress-message").innerHTML = `${message} <a href="/login.html">Sign in, then try again.</a>`;
      }
      return;
    }
    await loadUpdateProgress();
  } catch (error) {
    updateHold = false;
    stopUpdateProgressPoll();
    renderUpdateProgress({ state: "error", percent: 0, message: String(error.message || error) });
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

function when(ms) {
  if (!ms) return "";
  return new Date(ms).toLocaleString([], { dateStyle: "medium", timeStyle: "short" });
}

function renderPeople(view) {
  const people = view.people || [];
  $("people-list").replaceChildren(...(people.length ? people.map((person) => {
    const item = document.createElement("li");
    item.textContent = person.name;
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = "Remove";
    button.addEventListener("click", async () => {
      $("people-status").textContent = "Removing…";
      const { res, message } = await postJson("/api/auth/people/remove", { name: person.name });
      $("people-status").textContent = message;
      if (res.ok) await loadPeople();
    });
    item.append(" ", button);
    return item;
  }) : [Object.assign(document.createElement("li"), { textContent: "No one yet. Add the first person on this screen." })]));
  const visits = view.visits || [];
  $("people-visits").replaceChildren(...(visits.length ? visits.map((visit) => {
    const item = document.createElement("li");
    item.textContent = `${visit.name} · ${when(visit.at_ms)}`;
    return item;
  }) : [Object.assign(document.createElement("li"), { textContent: "No one has signed in yet." })]));
}

async function loadPeople() {
  const res = await fetch("/api/auth/people", { credentials: "same-origin" });
  if (!res.ok) {
    $("people-status").textContent = "Sign in on this screen, then add a person.";
    return;
  }
  renderPeople(await res.json());
}

async function loadAppSignins() {
  const list = $("app-signins-list");
  try {
    const res = await fetch("/api/apps/signins", { credentials: "same-origin" });
    if (!res.ok) throw new Error("Sign in as an administrator to manage app sign-ins.");
    const signins = await res.json();
    list.replaceChildren(...(signins.length ? signins.map((signin) => {
      const item = document.createElement("li");
      const label = document.createElement("span");
      label.textContent = `${signin.title} · ${signin.user}`;
      const forget = document.createElement("button");
      forget.type = "button";
      forget.textContent = "Forget";
      forget.addEventListener("click", async () => {
        forget.disabled = true;
        try {
          const { res: result, message } = await postJson(`/api/apps/${encodeURIComponent(signin.id)}/forget`, {});
          $("app-signins-status").textContent = message;
          if (result.ok) await loadAppSignins();
        } catch (error) {
          $("app-signins-status").textContent = String(error);
          forget.disabled = false;
        }
      });
      item.append(label, " ", forget);
      return item;
    }) : [Object.assign(document.createElement("li"), { textContent: "No saved app sign-ins." })]));
  } catch (error) {
    list.replaceChildren(Object.assign(document.createElement("li"), { textContent: String(error.message || error) }));
  }
}

$("app-signin-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const app = $("app-signin-app").value;
  const user = $("app-signin-user").value.trim();
  const password = $("app-signin-pass").value;
  const status = $("app-signins-status");
  status.textContent = "Checking with the app…";
  try {
    const { res, message } = await postJson(`/api/apps/${encodeURIComponent(app)}/remember`, { user, password });
    status.textContent = message;
    if (res.ok) {
      $("app-signin-pass").value = "";
      await loadAppSignins();
    }
  } catch (error) {
    status.textContent = String(error);
  }
});

$("people-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const name = $("people-name").value.trim();
  const password = $("people-pass").value;
  const again = $("people-pass2").value;
  if (!name || !password) {
    $("people-status").textContent = "Type a name and a password.";
    return;
  }
  if (password !== again) {
    $("people-status").textContent = "Type the same password again.";
    return;
  }
  $("people-status").textContent = "Saving…";
  const { res, message } = await postJson("/api/auth/people", { name, password });
  $("people-status").textContent = message;
  if (!res.ok) return;
  $("people-name").value = "";
  $("people-pass").value = "";
  $("people-pass2").value = "";
  await loadPeople();
});

async function loadWake() {
  try {
    const res = await fetch("/api/power/wake", { credentials: "same-origin" });
    if (!res.ok) return;
    const data = await res.json();
    if ($("wake-status") && !$("wake-status").textContent) $("wake-status").textContent = data.message || "";
  } catch (_) { /* The sentence from the button stays. */ }
}

$("wake-prepare").addEventListener("click", async () => {
  $("wake-prepare").disabled = true;
  $("wake-status").textContent = "Preparing wake. This can take a minute the first time.";
  try {
    const { message } = await postJson("/api/power/wake", {});
    $("wake-status").textContent = message || "Wake stayed as it was.";
  } catch (err) {
    $("wake-status").textContent = String(err);
  }
  $("wake-prepare").disabled = false;
});

// Light or full, per app. Keel's suggestion comes from this computer's memory.
async function loadVersions() {
  const box = $("versions-list");
  let list = [];
  try {
    const res = await fetch("/api/versions", { credentials: "same-origin" });
    list = res.ok ? await res.json() : [];
  } catch {
    list = [];
  }
  if (!list.length) {
    box.innerHTML = '<p class="meta">Keel could not read this computer.</p>';
    return;
  }
  box.replaceChildren(...list.map((app) => {
    const row = document.createElement("fieldset");
    row.className = "version-row";
    const legend = document.createElement("legend");
    legend.textContent = app.title;
    row.appendChild(legend);
    for (const [version, name, is, mb] of [["light", app.light, app.light_is, app.light_mb], ["full", app.full, app.full_is, app.full_mb]]) {
      const label = document.createElement("label");
      label.className = "version-option";
      const input = document.createElement("input");
      input.type = "radio";
      input.name = `version-${app.app}`;
      input.value = version;
      input.checked = app.chosen === version;
      input.disabled = version === "light" && !app.light_ready;
      input.addEventListener("change", () => chooseVersion(app.app, version));
      const text = document.createElement("span");
      const title = document.createElement("strong");
      title.textContent = `${version === "light" ? "Light" : "Full"}: ${name}`;
      const size = document.createElement("span");
      size.className = "meta";
      const notes = [`about ${mb} MB`];
      if (app.suggested === version) notes.push("Suggested for this computer");
      if (version === "light" && !app.light_ready) notes.push("Comes with the next Keel update");
      size.textContent = ` ${notes.join(" · ")}`;
      const line = document.createElement("span");
      line.className = "meta version-is";
      line.textContent = is;
      text.append(title, size, line);
      label.append(input, text);
      row.appendChild(label);
    }
    const why = document.createElement("p");
    why.className = "meta";
    why.textContent = app.why;
    row.appendChild(why);
    return row;
  }));
}

async function chooseVersion(app, version) {
  $("versions-status").textContent = "Keeping that.";
  try {
    const { message } = await postJson("/api/versions", { app, version });
    $("versions-status").textContent = message || "";
  } catch (err) {
    $("versions-status").textContent = String(err.message || err);
  }
  await loadVersions();
}

async function loadOwnedName() {
  const input = $("owned-input");
  if (!input) return;
  const res = await fetch("/api/name", { credentials: "same-origin" });
  if (!res.ok) return;
  const data = await res.json();
  if (data.name) {
    input.value = data.name;
    $("owned-status").textContent = `Apps will use names like ${data.photos}.`;
  }
}

$("owned-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const status = $("owned-status");
  status.textContent = "Saving.";
  try {
    const res = await fetch("/api/name", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify({ name: $("owned-input").value }),
    });
    const data = await res.json().catch(() => ({ message: "Keel could not store the name." }));
    if (!res.ok || !data.ok) {
      status.textContent = data.message || "Keel could not store the name.";
      return;
    }
    status.textContent = data.photos ? `Apps will use names like ${data.photos}.` : "Saved.";
  } catch (err) {
    status.textContent = String(err);
  }
});

loadOwnedName();
loadVersions();
loadWake();
loadPeople();
loadAppSignins();
loadUpdates();
loadUpdateProgress();
refreshConnect();

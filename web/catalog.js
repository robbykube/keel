const $ = (id) => document.getElementById(id);

function escapeHtml(value) {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

// The title of a card and, when it has one, how the app is right now.
function head(title, state, tone = "") {
  const pill = state ? `<span class="pill${tone ? ` ${tone}` : ""}">${escapeHtml(state)}</span>` : "";
  return `<div class="app-head"><h3>${escapeHtml(title)}</h3>${pill}</div>`;
}

function row(app, added) {
  const action = added
    ? ""
    : `<button type="button" data-app="${escapeHtml(app.id)}">Add</button>`;
  const note = added
    ? "Added. It stays closed until the program is on this computer."
    : app.why;
  return `<article class="app-row">
    <div>
      ${head(app.role, added ? "Closed" : "")}
      <p>${escapeHtml(note)}</p>
    </div>
    ${action}
  </article>`;
}

function stateOf(status, waiting) {
  if (status.ready) return ["Running", "ok"];
  if (status.state === "running") return ["Opening", ""];
  if (status.state === "error") return ["Stopped", "bad"];
  if (status.state === "unavailable") return ["Not on this computer", ""];
  return [waiting ? "Waiting" : "Closed", ""];
}

const programs = {
  names: { id: "names", title: "Names", path: "/api/apps/technitium", host: "names" },
  door: { id: "door", title: "Front door", path: "/api/apps/npm", host: "names" },
  video: { id: "video", title: "Video", path: "/api/apps/jitsi", host: "added" },
  passwords: { id: "passwords", title: "Passwords", path: "/api/apps/vaultwarden", host: "added" },
  papers: { id: "papers", title: "Papers", path: "/api/apps/papra", host: "added" },
  mail: { id: "mail", title: "Mail", path: "/api/apps/thunderbird", host: "added" },
};
const chosenProgram = { jitsi: "video", vaultwarden: "passwords", immich: "photos", papra: "papers", thunderbird: "mail" };
const programTimers = {};
const latest = {};
let ownedName = "";
let sharedReady = false;

function programRow(spec, status) {
  const failed = status.state === "error";
  const opening = status.state === "running";
  // The meeting link is what people send each other, so it can be copied.
  const copy = spec.id === "video"
    ? ` <button type="button" class="texty" data-copy="${escapeHtml(status.address || "")}">Copy link</button>`
    : "";
  const address = status.ready && status.address
    ? `<p><a href="${escapeHtml(status.address)}">${escapeHtml(status.address)}</a>${copy}</p>`
    : "";
  const percent = Math.max(0, Math.min(100, Number(status.percent) || 0));
  const bar = opening
    ? `<div class="install-bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${percent}"><span style="width:${percent}%"></span></div>`
    : "";
  const waiting = spec.host === "added" && !sharedReady && !status.ready;
  const sharedDown = !latest.names || !latest.names.ready || !latest.door || !latest.door.ready;
  const message = waiting && status.state !== "error" && status.state !== "running"
    ? (sharedDown ? "Closed until Names and the front door are ready." : "Closed until the name is saved.")
    : (status.message || "");
  const shared = spec.host === "names" && status.state !== "error";
  const open = status.state === "unavailable" || status.ready || waiting || shared
    ? ""
    : `<button type="button" data-program="${escapeHtml(spec.id)}"${opening ? " disabled" : ""}>Open</button>`;
  const close = status.ready && status.signups_open
    ? `<button type="button" data-close="${escapeHtml(spec.id)}">Close new accounts</button>`
    : "";
  const stop = spec.id === "video" && status.ready
    ? `<button type="button" data-stop="${escapeHtml(spec.id)}">Close</button>`
    : "";
  const [state, tone] = stateOf(status, waiting);
  return `<article class="app-row" id="${escapeHtml(spec.id)}-row">
    <div>
      ${head(spec.title, state, tone)}
      <p class="install-message${failed ? " install-error" : ""}">${escapeHtml(message)}</p>
      ${address}
      ${bar}
    </div>
    ${open}${close}${stop}
  </article>`;
}

function paintProgram(spec, status) {
  latest[spec.id] = status;
  const host = $(spec.host);
  if (!host) return;
  const existing = $(`${spec.id}-row`);
  const html = programRow(spec, status);
  if (existing) existing.outerHTML = html;
  else if (spec.host === "added") host.insertAdjacentHTML("afterbegin", html);
  else host.insertAdjacentHTML("beforeend", html);
  if (spec.id === "names" || spec.id === "door") updateGate();
}

function updateGate() {
  const was = sharedReady;
  const names = latest.names;
  const door = latest.door;
  sharedReady = !!ownedName && !!(names && names.ready) && !!(door && door.ready);
  const note = $("apps-wait");
  if (note) {
    const failed = (names && names.state === "error") || (door && door.state === "error");
    if (!names || !names.ready || !door || !door.ready) {
      note.textContent = failed
        ? "Names or the front door stopped. Sign in, then open Logs. Apps stay closed until both are ready."
        : "Names and the front door open on their own. Apps stay closed until they are ready.";
    } else if (!ownedName) note.textContent = "Save the name you own. You can change it later. Apps stay closed until it is saved.";
    else note.textContent = "";
  }
  if (was === sharedReady) return;
  for (const spec of Object.values(programs)) {
    if (spec.host === "added" && latest[spec.id]) paintProgram(spec, latest[spec.id]);
  }
  if ($("photos-row")) paintPhotos();
}

async function programStatus(spec) {
  const res = await fetch(spec.path, { credentials: "same-origin" });
  if (!res.ok) return null;
  return res.json();
}

async function readAnswer(res) {
  const text = await res.text();
  if (!text) return {};
  try {
    const data = JSON.parse(text);
    return data && typeof data === "object" ? data : { message: text.trim() };
  } catch {
    return { message: text.trim() };
  }
}

function watchProgram(spec) {
  if (programTimers[spec.id]) window.clearTimeout(programTimers[spec.id]);
  let quiet = 0;
  const tick = async () => {
    programTimers[spec.id] = 0;
    const status = await programStatus(spec);
    if (!status || status.state === "idle") {
      if (spec.host === "names") {
        if (status) paintProgram(spec, status);
        programTimers[spec.id] = window.setTimeout(tick, 1500);
        return;
      }
      quiet += 1;
      if (quiet < 8) programTimers[spec.id] = window.setTimeout(tick, 800);
      else if (status) paintProgram(spec, status);
      return;
    }
    quiet = 0;
    paintProgram(spec, status);
    if (status.state === "running") programTimers[spec.id] = window.setTimeout(tick, 800);
  };
  programTimers[spec.id] = window.setTimeout(tick, 400);
}

async function openProgram(spec) {
  if (programTimers[spec.id]) window.clearTimeout(programTimers[spec.id]);
  const opening = { state: "running", percent: 8, message: `Opening ${spec.title}. This can take a while.`, ready: false, address: "" };
  paintProgram(spec, opening);
  try {
    const res = await fetch(`${spec.path}/install`, { method: "POST", credentials: "same-origin" });
    const data = await readAnswer(res);
    if (!res.ok || data.ok === false) {
      paintProgram(spec, { state: "error", percent: 0, message: data.message || `Keel could not open ${spec.title}.`, ready: false, address: "" });
      return;
    }
    if (data.ready) {
      paintProgram(spec, { state: "done", percent: 100, message: data.message, ready: true, address: data.address, signups_open: data.signups_open });
      return;
    }
    paintProgram(spec, {
      state: data.state || "running",
      percent: Number(data.percent) || opening.percent,
      message: data.message || opening.message,
      ready: false,
      address: data.address || "",
    });
    watchProgram(spec);
  } catch (err) {
    paintProgram(spec, { state: "error", percent: 0, message: String(err), ready: false, address: "" });
  }
}

async function closeProgram(spec) {
  if (programTimers[spec.id]) window.clearTimeout(programTimers[spec.id]);
  paintProgram(spec, { state: "running", percent: 2, message: "Closing new accounts.", ready: false, address: "" });
  try {
    const res = await fetch(`${spec.path}/close`, { method: "POST", credentials: "same-origin" });
    const data = await readAnswer(res);
    if (!res.ok || !data.ok) {
      paintProgram(spec, { state: "error", percent: 0, message: data.message || "Keel could not close new accounts.", ready: false, address: "" });
      return;
    }
    if (data.ready) {
      paintProgram(spec, { state: "done", percent: 100, message: data.message, ready: true, address: data.address, signups_open: data.signups_open });
      return;
    }
    watchProgram(spec);
  } catch (err) {
    paintProgram(spec, { state: "error", percent: 0, message: String(err), ready: false, address: "" });
  }
}

function photosCard(status) {
  const allowed = !!(status && status.allowed);
  const opening = allowed && status.state === "running";
  const failed = allowed && status.state === "error";
  const ready = allowed && status.ready;
  const address = ready && status.address
    ? `<p><a href="${escapeHtml(status.address)}">${escapeHtml(status.address)}</a></p>`
    : "";
  const percent = Math.max(0, Math.min(100, Number(status && status.percent) || 0));
  const bar = opening
    ? `<div class="install-bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${percent}"><span style="width:${percent}%"></span></div>`
    : "";
  let full = `<p class="app-warn">Jellyfin needs a larger computer.</p>`;
  let jelly = "";
  if (allowed && status.state === "unavailable") {
    full = `<p class="install-message">${escapeHtml(status.message || "Jellyfin installs on the Keel computer.")}</p>`;
  } else if (allowed && !sharedReady && !opening && !failed && !ready) {
    full = `<p class="install-message">Jellyfin stays closed until Names and the front door are ready.</p>`;
  } else if (allowed) {
    full = `<p class="install-message${failed ? " install-error" : ""}">${escapeHtml(status.message || "")}</p>${address}${bar}`;
    jelly = ready
      ? ""
      : `<button type="button" data-program="jellyfin"${opening ? " disabled" : ""}>Jellyfin</button>`;
  }
  return `<article class="app-row" id="photos-row">
    <div>
      ${head("Photos", "Ready", "ok")}
      <p class="install-message">Pictures already in Media. Open shows them here.</p>
      ${full}
    </div>
    <a class="button" href="/photos.html">Open</a>
    ${jelly}
  </article>`;
}

async function paintPhotos() {
  let status = { allowed: false, state: "refused", message: "Jellyfin needs a larger computer." };
  try {
    const res = await fetch("/api/apps/jellyfin", { credentials: "same-origin" });
    if (res.ok) status = await res.json();
  } catch {
    /* The sentence stays the refusal. */
  }
  const host = $("added");
  if (!host) return;
  const html = photosCard(status);
  const existing = $("photos-row");
  if (existing) existing.outerHTML = html;
  else host.insertAdjacentHTML("afterbegin", html);
  if (status.state === "running") {
    window.setTimeout(paintPhotos, 800);
  }
}

async function openJellyfin() {
  const host = $("added");
  if (!host || !$("photos-row")) return;
  $("photos-row").outerHTML = photosCard({
    allowed: true,
    state: "running",
    percent: 8,
    message: "Opening Jellyfin. This can take a while.",
    ready: false,
    address: "",
  });
  try {
    const res = await fetch("/api/apps/jellyfin/install", { method: "POST", credentials: "same-origin" });
    const data = await readAnswer(res);
    if (!res.ok || data.ok === false) {
      $("photos-row").outerHTML = photosCard({
        allowed: true,
        state: "error",
        percent: 0,
        message: data.message || "Keel could not open Jellyfin.",
        ready: false,
        address: "",
      });
      return;
    }
    $("photos-row").outerHTML = photosCard(data);
    if (!data.ready) window.setTimeout(paintPhotos, 800);
  } catch (err) {
    if ($("photos-row")) {
      $("photos-row").outerHTML = photosCard({
        allowed: true,
        state: "error",
        percent: 0,
        message: String(err),
        ready: false,
        address: "",
      });
    }
  }
}

// Closing ends any call in progress, so the first press asks.
function askBeforeStop(button) {
  if (button.dataset.sure === "1") return true;
  button.dataset.sure = "1";
  button.textContent = "End the call and close";
  button.classList.add("danger");
  window.setTimeout(() => {
    if (!button.isConnected) return;
    delete button.dataset.sure;
    button.textContent = "Close";
    button.classList.remove("danger");
  }, 4000);
  return false;
}

async function copyLink(button) {
  const link = button.dataset.copy;
  if (!link) return;
  let done = false;
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(link);
      done = true;
    }
  } catch (_) { /* tried the other way below */ }
  if (!done) {
    // A page on the house address is not a secure page, so the clipboard
    // is reached the older way.
    const box = document.createElement("textarea");
    box.value = link;
    box.setAttribute("readonly", "");
    box.style.position = "fixed";
    box.style.opacity = "0";
    document.body.appendChild(box);
    box.select();
    try { done = document.execCommand("copy"); } catch (_) { done = false; }
    box.remove();
  }
  button.textContent = done ? "Copied" : "Copy did not work. Hold the link instead.";
  window.setTimeout(() => { if (button.isConnected) button.textContent = "Copy link"; }, 2000);
}

async function stopVideo(spec) {
  if (programTimers[spec.id]) window.clearTimeout(programTimers[spec.id]);
  paintProgram(spec, { state: "running", percent: 2, message: "Closing Video.", ready: false, address: "" });
  try {
    const res = await fetch(`${spec.path}/close`, { method: "POST", credentials: "same-origin" });
    const data = await readAnswer(res);
    if (!res.ok || data.ok === false) {
      paintProgram(spec, { state: "error", percent: 0, message: data.message || "Keel could not close Video.", ready: false, address: "" });
      return;
    }
    paintProgram(spec, {
      state: data.state || "idle",
      percent: Number(data.percent) || 0,
      message: data.message || "Video is on this computer. Press Open when you want a call.",
      ready: false,
      address: "",
    });
  } catch (err) {
    paintProgram(spec, { state: "error", percent: 0, message: String(err), ready: false, address: "" });
  }
}

function onProgramClick(event) {
  const copy = event.target.closest("button[data-copy]");
  if (copy) {
    copyLink(copy);
    return;
  }
  const stop = event.target.closest("button[data-stop]");
  if (stop) {
    const spec = programs[stop.dataset.stop];
    if (spec && askBeforeStop(stop)) stopVideo(spec);
    return;
  }
  const close = event.target.closest("button[data-close]");
  if (close) {
    const spec = programs[close.dataset.close];
    if (spec) closeProgram(spec);
    return;
  }
  const open = event.target.closest("button[data-program]");
  if (!open) return;
  if (open.dataset.program === "jellyfin") {
    openJellyfin();
    return;
  }
  const spec = programs[open.dataset.program];
  if (spec) openProgram(spec);
}

async function loadAlways() {
  const host = $("names");
  if (host) host.innerHTML = "";
  for (const id of ["names", "door"]) {
    const spec = programs[id];
    const status = await programStatus(spec);
    if (!status) continue;
    paintProgram(spec, status);
    if (!status.ready && status.state !== "unavailable") watchProgram(spec);
  }
}

async function load() {
  const apps = await fetch("/api/apps", { credentials: "same-origin" }).then((res) => res.json());
  const choices = (Array.isArray(apps) ? apps : []).filter((app) => app.choosable);
  const special = new Set(Object.keys(chosenProgram));
  const added = choices.filter((app) => app.installed && !special.has(app.id));
  const opened = choices.filter((app) => app.installed && special.has(app.id));
  const more = choices.filter((app) => !app.installed);
  $("added").innerHTML = added.length
    ? added.map((app) => row(app, true)).join("")
    : (opened.length ? "" : `<p class="meta">None yet. Add one below, or pick the first ones on Home.</p>`);
  $("more").innerHTML = more.length
    ? more.map((app) => row(app, false)).join("")
    : `<p class="meta">Every app on the list is already added.</p>`;
  if (opened.some((app) => app.id === "immich")) paintPhotos();
  for (const appId of ["thunderbird", "papra", "vaultwarden", "jitsi"]) {
    if (!opened.some((app) => app.id === appId)) continue;
    const spec = programs[chosenProgram[appId]];
    const status = await programStatus(spec);
    if (!status) continue;
    paintProgram(spec, status);
    if (status.state === "running") watchProgram(spec);
  }
  await loadAlways();
  await loadOwned();
}

async function loadOwned() {
  const input = $("owned-input");
  const status = $("owned-status");
  if (!input || !status) return;
  const res = await fetch("/api/name", { credentials: "same-origin" });
  if (!res.ok) return;
  const data = await res.json();
  if (data.name) {
    input.value = data.name;
    ownedName = data.name;
    status.textContent = `Keel uses that name for each app, such as ${data.photos}. You can change it later.`;
  } else {
    ownedName = "";
  }
  updateGate();
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
    ownedName = data.name || $("owned-input").value.trim();
    status.textContent = `Keel uses that name for each app, such as ${data.photos}. You can change it later.`;
    updateGate();
  } catch (err) {
    status.textContent = String(err);
  }
});

$("names").addEventListener("click", onProgramClick);
$("added").addEventListener("click", onProgramClick);

$("more").addEventListener("click", async (event) => {
  const button = event.target.closest("button[data-app]");
  if (!button) return;
  button.disabled = true;
  $("status").textContent = "Adding.";
  try {
    const res = await fetch("/api/apps/choose", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify({ ids: [button.dataset.app] }),
    });
    const data = await res.json();
    if (!res.ok || !data.ok) {
      $("status").textContent = data.message || "Keel could not add that.";
      button.disabled = false;
      return;
    }
    $("status").textContent = "";
    await load();
  } catch (err) {
    $("status").textContent = String(err);
    button.disabled = false;
  }
});

load().catch((err) => {
  $("status").textContent = String(err);
});

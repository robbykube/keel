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
    ? "Added. Opens once its program is on this computer."
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
  if (status.local) return ["Ready", "ok"];
  if (status.ready) return ["Running", "ok"];
  if (status.state === "running") return ["Opening", ""];
  if (status.state === "error") return ["Stopped", "idle"];
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
  photos: { id: "photos", title: "Photos", path: "/api/apps/immich", host: "added" },
  watch: { id: "watch", title: "Watch", path: "/api/apps/jellyfin", host: "added" },
};
const chosenProgram = { jitsi: "video", vaultwarden: "passwords", immich: "photos", papra: "papers", thunderbird: "mail", jellyfin: "watch" };
const programTimers = {};
const latest = {};
const barMotion = {};
let barLoop = 0;
let ownedName = "";
let sharedReady = false;

// The server reports the start of a step. The bar eases toward the end of
// that step and stops just short of it, so a long download keeps moving
// without claiming it has finished.
function stageCap(floor) {
  if (floor >= 100) return 100;
  if (floor >= 92) return 99;
  return 90;
}

function motionOff() {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function shownPercent(id, status) {
  const floor = Math.max(0, Math.min(100, Number(status.percent) || 0));
  if (status.state !== "running") {
    delete barMotion[id];
    return floor;
  }
  if (motionOff()) return floor;
  let motion = barMotion[id];
  if (!motion || motion.floor !== floor) {
    const carried = motion && motion.shown > floor ? motion.shown : floor;
    motion = { floor, from: carried, started: performance.now(), shown: carried };
    barMotion[id] = motion;
  }
  const cap = stageCap(floor);
  const elapsed = (performance.now() - motion.started) / 1000;
  const value = motion.from + (cap - motion.from) * (1 - Math.pow(0.5, elapsed / 45));
  motion.shown = Math.min(cap - 0.4, Math.max(floor, value));
  return motion.shown;
}

function pumpBars() {
  barLoop = 0;
  let moving = false;
  for (const spec of Object.values(programs)) {
    const status = latest[spec.id];
    if (!status || status.state !== "running" || motionOff()) continue;
    moving = true;
    const value = shownPercent(spec.id, status);
    const row = $(`${spec.id}-row`);
    const span = row && row.querySelector(".install-bar > span");
    const track = row && row.querySelector(".install-bar");
    if (span) span.style.width = `${value}%`;
    if (track) track.setAttribute("aria-valuenow", String(Math.round(value)));
  }
  if (moving) barLoop = window.setTimeout(pumpBars, 120);
}

function keepBarsMoving() {
  if (!barLoop) pumpBars();
}

function onKeelScreen() {
  const host = location.hostname;
  return host === "127.0.0.1" || host === "localhost" || host === "::1" || host === "[::1]";
}

function programRow(spec, status) {
  const failed = status.state === "error";
  const opening = status.state === "running";
  // The meeting link is what people send each other. Passwords, Papers, and
  // Mail addresses are opened on a phone. On the Keel screen they stay as
  // text, so the page is not left behind.
  const copyLabel = spec.id === "video" ? "Copy link" : "Copy address";
  const copy = (spec.id === "video" || spec.id === "passwords" || spec.id === "papers" || spec.id === "mail") && status.address
    ? ` <button type="button" class="texty" data-copy="${escapeHtml(status.address)}" data-label="${escapeHtml(copyLabel)}">${copyLabel}</button>`
    : "";
  const stayHere = (spec.id === "passwords" || spec.id === "papers" || spec.id === "mail") && onKeelScreen();
  const address = status.ready && status.address
    ? (stayHere
      ? `<p>${escapeHtml(status.address)}${copy}</p>`
      : `<p><a href="${escapeHtml(status.address)}">${escapeHtml(status.address)}</a>${copy}</p>`)
    : "";
  const signIn = spec.id === "mail" && status.ready && status.password
    ? `<p>Sign in as mail. ${escapeHtml(status.password)} <button type="button" class="texty" data-copy="${escapeHtml(status.password)}" data-label="Copy password">Copy password</button></p>`
    : "";
  const percent = opening ? shownPercent(spec.id, status) : 0;
  const bar = opening
    ? `<div class="install-bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${Math.round(percent)}"><span style="width:${percent}%"></span></div>`
    : "";
  const waiting = !status.local && status.allowed !== false && spec.host === "added" && !sharedReady && !status.ready;
  const sharedDown = !latest.names || !latest.names.ready || !latest.door || !latest.door.ready;
  const message = status.local
    ? (status.message || "Pictures already in Media. Open shows them here.")
    : (waiting && status.state !== "error" && status.state !== "running"
      ? (sharedDown ? "Closed until Names and the front door are ready." : "Closed until the name is saved.")
      : (status.message || ""));
  // A raw error from the computer goes behind ?, with a sentence a person can act on.
  const said = failed ? String(status.message || "").trim() : "";
  const failText = failed ? `${spec.title} did not start. Try again, or open Logs to see why.` : "";
  const shared = spec.host === "names" && status.state !== "error";
  const open = status.local
    ? `<a class="button" href="/photos.html">Open</a>`
    : (status.allowed === false || status.state === "unavailable" || status.ready || waiting || shared
      ? ""
      : `<button type="button" data-program="${escapeHtml(spec.id)}"${opening ? " disabled" : ""}>${failed ? "Try again" : "Open"}</button>`);
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
      <p class="install-message">${failed ? `${escapeHtml(failText)} ${said ? keelHelp(said, "What the computer said") : ""}` : escapeHtml(message)}</p>
      ${address}
      ${signIn}
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
  if (status.state === "running") keepBarsMoving();
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
        ? "Apps wait until Names and the front door are running."
        : "Names and the front door are starting. Apps wait for them.";
    } else if (!ownedName) note.textContent = "Save the name you own in Settings. Apps wait for it.";
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
      paintProgram(spec, { state: "done", percent: 100, message: data.message, ready: true, address: data.address, signups_open: data.signups_open, password: data.password || "" });
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

async function paintPhotos() {
  const spec = programs.photos;
  const status = await programStatus(spec);
  if (!status) return;
  paintProgram(spec, status);
  if (status.state === "running") watchProgram(spec);
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
  window.setTimeout(() => { if (button.isConnected) button.textContent = button.dataset.label || "Copy link"; }, 2000);
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
    : (opened.length ? "" : `<p class="meta">None yet. Add one below.</p>`);
  $("more").innerHTML = more.length
    ? more.map((app) => row(app, false)).join("")
    : `<p class="meta">Every app on the list is already added.</p>`;
  if (opened.some((app) => app.id === "immich")) paintPhotos();
  for (const appId of ["thunderbird", "papra", "vaultwarden", "jitsi", "jellyfin"]) {
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
  const res = await fetch("/api/name", { credentials: "same-origin" });
  if (!res.ok) return;
  const data = await res.json();
  ownedName = data.name || "";
  updateGate();
}

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

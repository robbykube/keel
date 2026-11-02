const $ = (id) => document.getElementById(id);

function escapeHtml(value) {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function row(app, added) {
  const action = added
    ? `<span class="pill">Closed</span>`
    : `<button type="button" data-app="${escapeHtml(app.id)}">Add</button>`;
  const note = added
    ? "Added. It stays closed until the program is on this computer."
    : app.why;
  return `<article class="app-row">
    <div>
      <h3>${escapeHtml(app.role)}</h3>
      <p>${escapeHtml(note)}</p>
    </div>
    ${action}
  </article>`;
}

const programs = {
  names: { id: "names", title: "Names", path: "/api/apps/technitium", host: "names" },
  door: { id: "door", title: "Front door", path: "/api/apps/npm", host: "names" },
  video: { id: "video", title: "Video", path: "/api/apps/jitsi", host: "added" },
  passwords: { id: "passwords", title: "Passwords", path: "/api/apps/vaultwarden", host: "added" },
  photos: { id: "photos", title: "Photos", path: "/api/apps/immich", host: "added" },
  papers: { id: "papers", title: "Papers", path: "/api/apps/papra", host: "added" },
  mail: { id: "mail", title: "Mail", path: "/api/apps/thunderbird", host: "added" },
};
const chosenProgram = { jitsi: "video", vaultwarden: "passwords", immich: "photos", papra: "papers", thunderbird: "mail" };
const programTimers = {};

function programRow(spec, status) {
  const failed = status.state === "error";
  const opening = status.state === "running";
  const address = status.ready && status.address
    ? `<p><a href="${escapeHtml(status.address)}">${escapeHtml(status.address)}</a></p>`
    : "";
  const percent = Math.max(0, Math.min(100, Number(status.percent) || 0));
  const bar = opening
    ? `<div class="install-bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${percent}"><span style="width:${percent}%"></span></div>`
    : "";
  const open = status.state === "unavailable" || status.ready
    ? ""
    : `<button type="button" data-program="${escapeHtml(spec.id)}"${opening ? " disabled" : ""}>Open</button>`;
  const close = status.ready && status.signups_open
    ? `<button type="button" data-close="${escapeHtml(spec.id)}">Close new accounts</button>`
    : "";
  return `<article class="app-row" id="${escapeHtml(spec.id)}-row">
    <div>
      <h3>${escapeHtml(spec.title)}</h3>
      <p class="install-message${failed ? " install-error" : ""}">${escapeHtml(status.message || "")}</p>
      ${address}
      ${bar}
    </div>
    ${open}${close}
  </article>`;
}

function paintProgram(spec, status) {
  const host = $(spec.host);
  if (!host) return;
  const existing = $(`${spec.id}-row`);
  const html = programRow(spec, status);
  if (existing) existing.outerHTML = html;
  else if (spec.host === "added") host.insertAdjacentHTML("afterbegin", html);
  else host.insertAdjacentHTML("beforeend", html);
}

async function programStatus(spec) {
  const res = await fetch(spec.path, { credentials: "same-origin" });
  if (!res.ok) return null;
  return res.json();
}

function watchProgram(spec) {
  if (programTimers[spec.id]) window.clearTimeout(programTimers[spec.id]);
  const tick = async () => {
    programTimers[spec.id] = 0;
    const status = await programStatus(spec);
    if (!status) return;
    paintProgram(spec, status);
    if (status.state === "running") programTimers[spec.id] = window.setTimeout(tick, 800);
  };
  programTimers[spec.id] = window.setTimeout(tick, 400);
}

async function openProgram(spec) {
  if (programTimers[spec.id]) window.clearTimeout(programTimers[spec.id]);
  paintProgram(spec, { state: "running", percent: 2, message: `Opening ${spec.title}.`, ready: false, address: "" });
  try {
    const res = await fetch(`${spec.path}/install`, { method: "POST", credentials: "same-origin" });
    const data = await res.json().catch(() => ({ message: `Keel could not open ${spec.title}.` }));
    if (!res.ok || !data.ok) {
      paintProgram(spec, { state: "error", percent: 0, message: data.message || `Keel could not open ${spec.title}.`, ready: false, address: "" });
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

async function closeProgram(spec) {
  if (programTimers[spec.id]) window.clearTimeout(programTimers[spec.id]);
  paintProgram(spec, { state: "running", percent: 2, message: "Closing new accounts.", ready: false, address: "" });
  try {
    const res = await fetch(`${spec.path}/close`, { method: "POST", credentials: "same-origin" });
    const data = await res.json().catch(() => ({ message: "Keel could not close new accounts." }));
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

function onProgramClick(event) {
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
    if (status.state === "running") watchProgram(spec);
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
  for (const appId of ["thunderbird", "papra", "immich", "vaultwarden", "jitsi"]) {
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
    status.textContent = `Keel uses that name for each app, such as ${data.photos}. At the place you bought the name, send it to this computer once.`;
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
    status.textContent = `Keel uses that name for each app, such as ${data.photos}. At the place you bought the name, send it to this computer once.`;
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

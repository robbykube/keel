const $ = (id) => document.getElementById(id);

function bytes(n) {
  if (!n) return "—";
  const u = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
  let i = 0;
  let x = n;
  while (x >= 1024 && i < u.length - 1) {
    x /= 1024;
    i++;
  }
  return `${x.toFixed(i ? 1 : 0)} ${u[i]}`;
}

function pct(n) {
  return `${n.toFixed(0)}%`;
}

async function get(path) {
  const t0 = performance.now();
  const res = await fetch(path);
  const data = await res.json();
  const client = Math.round(performance.now() - t0);
  const server = res.headers.get("server-timing") || "";
  return { data, client, server };
}

function escapeHtml(value) {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function plainSize(n) {
  if (!n) return { number: "—", unit: "" };
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  let x = n;
  while (x >= 1000 && i < units.length - 1) {
    x /= 1000;
    i++;
  }
  const digits = i && x < 10 ? 1 : 0;
  return { number: x.toFixed(digits), unit: units[i] };
}

function greeting(date) {
  const hour = date.getHours();
  if (hour < 5 || hour >= 17) return "Good evening";
  if (hour < 12) return "Good morning";
  return "Good afternoon";
}

function afterSignIn(path, signedIn) {
  return signedIn ? path : `/login.html?next=${encodeURIComponent(path)}`;
}

function smbUrl(unc) {
  const match = String(unc).match(/^\\\\([^\\]+)\\(.+)$/);
  if (!match) return unc;
  return `smb://${match[1]}/${match[2].replace(/\\/g, "/")}`;
}

function filesShare(shares) {
  const list = Array.isArray(shares) ? shares : [];
  return list.find((share) => share.name === "files") || list[0] || null;
}

function card(k, v) {
  return `<div class="card"><div class="k">${k}</div><div class="v">${v}</div></div>`;
}

function filesCard(share) {
  const ready = Boolean(share && share.ready && share.lan && share.lan[0]);
  const path = ready ? share.lan[0] : "";
  const mac = path ? smbUrl(path) : "";
  const body = ready
    ? `<div class="path-box"><code>${escapeHtml(path)}</code><button type="button" class="copy-button" id="copy-path" data-path="${escapeHtml(path)}" aria-label="Copy the files address">Copy <span aria-hidden="true">↗</span></button></div>
       <dl>
         <div class="pair"><dt>Windows</dt><dd>Paste into File Explorer</dd></div>
         <div class="pair"><dt>Mac</dt><dd>Finder → Go → Connect to Server, ${escapeHtml(mac)}</dd></div>
         <div class="pair"><dt>Phone</dt><dd>Files app → Connect to server → same address</dd></div>
       </dl>`
    : `<p>The family folder is not ready on this computer yet.</p>
       <a class="button push" href="/shares.html">Open Files</a>`;
  return `<article class="card home-card">
    <div class="home-card-head">
      <h2 class="home-title"><span class="home-icon" aria-hidden="true">⌁</span><span>Files</span></h2>
      <span class="pill ${ready ? "ok" : ""}">${ready ? "Ready" : "Not set up"}</span>
    </div>
    <p>Open the family folder from any computer in the house.</p>
    ${body}
  </article>`;
}

function awayCard(overlay, owner) {
  const state = overlay && overlay.state;
  const ready = state === "connected";
  const who = owner
    ? `${owner} connects Keel once`
    : "The person who set up this computer connects Keel once";
  const pill = ready ? "Ready" : state === "unavailable" ? "Not on this computer" : "Not set up";
  return `<article class="card home-card">
    <div class="home-card-head">
      <h2 class="home-title"><span class="home-icon" aria-hidden="true">◎</span><span>Away</span></h2>
      <span class="pill ${ready ? "ok" : ""}">${escapeHtml(pill)}</span>
    </div>
    <p>Open the same files when you are not at home. Only the phones and laptops you trust can reach them.</p>
    <ol class="steps">
      <li><span>1</span>${escapeHtml(who)}</li>
      <li><span>2</span>Use the same address anywhere</li>
    </ol>
    <a class="button push" href="/overlay.html">How Away works</a>
  </article>`;
}

function roomBar(disk) {
  const total = disk.fs_size || disk.size || 0;
  if (disk.free == null || !total) return "";
  const left = Math.max(0, Math.min(100, Math.round((disk.free / total) * 100)));
  const failing = disk.health === "failing";
  const filling = left < 15;
  const word = failing ? "Failing" : filling ? "Filling up" : "";
  return `<div class="room${failing ? " failing" : ""}" role="img" aria-label="${left}% free"><span style="width:${left}%"></span></div>${word ? `<div class="room-word${failing ? " failing" : ""}">${word}</div>` : ""}`;
}

function disksCard(storage, failing) {
  const disks = (storage.disks || []).map((row) => row.disk ? { ...row.disk, holds: row.holds } : row);
  const total = disks.reduce((sum, disk) => sum + (disk.size || 0), 0);
  const size = plainSize(total);
  const rows = disks.map((disk) => {
    const name = disk.model || disk.name || "Disk";
    const extra = disk.transport === "usb" ? " · USB" : "";
    const shown = plainSize(disk.size);
    const failingDisk = disk.health === "failing";
    return `<div class="disk-row${failingDisk ? " failing" : ""}">
      <div class="name">${escapeHtml(name)}</div>
      <div class="size">${escapeHtml(shown.number)} ${escapeHtml(shown.unit)}</div>
      <div class="holds">${escapeHtml(disk.holds || "")}${escapeHtml(extra)}</div>
      ${roomBar(disk)}
    </div>`;
  }).join("");
  const headline = disks.length
    ? `<p class="display">${escapeHtml(size.number)} <span>${escapeHtml(size.unit)}</span></p><p>in this computer</p>`
    : `<p>No disks yet.</p>`;
  return `<article class="card home-card">
    <div class="home-card-head">
      <h2 class="home-title"><span class="home-icon" aria-hidden="true">▣</span><span>Disks</span></h2>
      <span class="pill ${failing ? "bad" : ""}">${failing ? "Needs a look" : (disks.length === 1 ? "1 disk" : `${disks.length} disks`)}</span>
    </div>
    ${headline}
    ${disks.length ? '<p class="card-note">The number above is how much the disks hold. A short bar is a disk filling up.</p>' : ''}
    <div class="disk-list">${rows}</div>
  </article>`;
}

function showNotice(lines, href, failing) {
  const box = $("notice");
  if (!lines.length) {
    box.hidden = true;
    box.replaceChildren();
    return;
  }
  const title = lines.length === 1 ? "1 thing to look at" : `${lines.length} things to look at`;
  box.className = failing ? "notice bad" : "notice";
  box.hidden = false;
  box.innerHTML = `
    <div class="notice-main">
      <span class="notice-dot" aria-hidden="true"></span>
      <div>
        <p class="notice-title">${escapeHtml(title)}</p>
        <p>${lines.map(escapeHtml).join(" ")}</p>
      </div>
    </div>
    <a class="button" href="${escapeHtml(href)}">Fix it</a>`;
}

function showAppChoice(apps) {
  const box = $("apps-choice");
  const again = $("apps-again");
  const rows = $("apps-choice-rows");
  if (!box || !rows) return;
  const choices = (Array.isArray(apps) ? apps : []).filter((app) => app.choosable);
  const any = choices.some((app) => app.installed);
  const later = sessionStorage.getItem("keel-apps-later") === "1";
  if (any || !choices.length) {
    box.hidden = true;
    again.hidden = true;
    return;
  }
  rows.innerHTML = choices.map((app) => `<label class="check">
      <input type="checkbox" name="app" value="${escapeHtml(app.id)}">
      <span><strong>${escapeHtml(app.role)}</strong> ${escapeHtml(app.why)}</span>
    </label>`).join("");
  const use = $("apps-use");
  const refresh = () => {
    use.disabled = !rows.querySelector("input:checked");
  };
  rows.onchange = refresh;
  refresh();
  box.hidden = later;
  again.hidden = !later;
}

async function load() {
  const [sys, health, issues, storage, shares, overlay, session, apps] = await Promise.all([
    get("/api/system"),
    get("/api/health"),
    get("/api/issues"),
    get("/api/storage"),
    get("/api/shares"),
    get("/api/overlay"),
    get("/api/auth/session"),
    get("/api/apps"),
  ]);

  const s = sys.data;
  const signedIn = Boolean(session.data.signed_in);
  const now = new Date();
  $("kicker").textContent = `${now.toLocaleDateString([], { weekday: "long" })} · ${s.hostname || "Keel"}`;
  $("title").textContent = greeting(now);
  $("health").textContent = issues.data.red
    ? "Needs a look"
    : `v${health.data.version}`;
  $("rest").innerHTML = `<a href="/sleep.html">Let this computer rest</a> when you are done.`;

  const filesRole = (storage.data.roles || []).find((role) => role.id === "files");
  const lines = [];
  let fix = afterSignIn("/pools.html", signedIn);
  if (!filesRole || !filesRole.ready) {
    lines.push(s.platform === "linux"
      ? "No disk has been picked for files yet. Files are saved on the system disk for now."
      : "No disk has been picked for files yet.");
  }
  const red = (issues.data.items || []).filter((item) => item.severity === "red");
  if (red.length) {
    red.forEach((item) => lines.push(item.title));
    fix = afterSignIn("/repair.html", signedIn);
  }
  showNotice(lines, fix, red.length > 0);
  showAppChoice(apps.data);

  const owner = storage.data.owner || "";
  $("home").innerHTML = [
    filesCard(filesShare(shares.data)),
    awayCard(overlay.data, owner),
    disksCard(storage.data, red.length > 0),
  ].join("");

  const copy = document.getElementById("copy-path");
  if (copy) {
    copy.addEventListener("click", async () => {
      const path = copy.getAttribute("data-path") || "";
      try {
        await navigator.clipboard.writeText(path);
        copy.textContent = "Copied";
      } catch {
        copy.textContent = "Select the address and copy it";
      }
      window.setTimeout(() => { copy.textContent = "Copy"; }, 1600);
    });
  }

  $("detail-body").innerHTML = `<div class="cards">${[
    card("CPU", `${pct(s.cpu_pct)} · ${s.cores} cores`),
    card("Memory", `${bytes(s.mem_used)} / ${bytes(s.mem_total)}`),
    card(`Daemon <a class="term" href="/guide.html#rss">RSS</a>`, bytes(s.daemon_rss)),
    card("Version", escapeHtml(health.data.version)),
  ].join("")}</div>
    <p class="meta">system ${escapeHtml(sys.server)} / ${sys.client}ms</p>`;
}

$("apps-choice-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const ids = [...$("apps-choice-rows").querySelectorAll("input:checked")].map((input) => input.value);
  const status = $("apps-choice-status");
  status.textContent = "Saving.";
  try {
    const res = await fetch("/api/apps/choose", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify({ ids }),
    });
    const data = await res.json();
    if (!res.ok || !data.ok) {
      status.textContent = data.message || "Keel could not save that choice.";
      return;
    }
    sessionStorage.removeItem("keel-apps-later");
    location.reload();
  } catch (err) {
    status.textContent = String(err);
  }
});

$("apps-later").addEventListener("click", () => {
  sessionStorage.setItem("keel-apps-later", "1");
  $("apps-choice").hidden = true;
  $("apps-again").hidden = false;
});

$("apps-show").addEventListener("click", () => {
  sessionStorage.removeItem("keel-apps-later");
  $("apps-choice").hidden = false;
  $("apps-again").hidden = true;
});

load().catch((err) => {
  $("health").textContent = String(err);
});

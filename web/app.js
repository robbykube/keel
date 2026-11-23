const $ = (id) => document.getElementById(id);

// The same line icons as the sidebar, so a card and its link look alike.
function homeIcon(name) {
  return (typeof NAV_ICONS !== "undefined" && NAV_ICONS[name]) || "";
}

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
  const how = `Windows: paste it into the File Explorer address bar. Mac: Finder, Go, Connect to Server, ${mac || "smb://…"}. Phone: the Files app, Connect to server.`;
  const body = ready
    ? `<div class="path-box"><code>${escapeHtml(path)}</code><button type="button" class="copy-button" id="copy-path" data-path="${escapeHtml(path)}" aria-label="Copy the files address">Copy</button></div>
       <p class="meta">Open it in File Explorer or Finder ${keelHelp(how, "How to open it")}</p>`
    : `<p>Give a disk the job of keeping files first.</p>
       <a class="button push" href="/pools.html">Choose a disk</a>`;
  return `<article class="card home-card">
    <div class="home-card-head">
      <h2 class="home-title"><span class="home-icon" aria-hidden="true">${homeIcon("Files")}</span><span>Files</span></h2>
      <span class="pill ${ready ? "ok" : ""}">${ready ? "Ready" : "Not set up"}</span>
    </div>
    ${body}
  </article>`;
}

function awayCard(overlay, owner) {
  const state = overlay && overlay.state;
  const ready = state === "connected";
  const pill = ready ? "Ready" : state === "unavailable" ? "Not on this computer" : "Not set up";
  const who = owner ? `${owner} connects` : "The person who set up Keel connects";
  const tip = `${who} this computer once. After that, the same Files address works on a laptop or phone outside the house.`;
  return `<article class="card home-card">
    <div class="home-card-head">
      <h2 class="home-title"><span class="home-icon" aria-hidden="true">${homeIcon("Away")}</span><span>Away</span> ${keelHelp(tip)}</h2>
      <span class="pill ${ready ? "ok" : ""}">${escapeHtml(pill)}</span>
    </div>
    <p>${ready ? "Your files open outside the house too." : "Open your files outside the house."}</p>
    <a class="button push" href="/overlay.html">${ready ? "Open Away" : "Set up Away"}</a>
  </article>`;
}

function roomBar(disk) {
  const total = disk.fs_size || disk.size || 0;
  if (disk.free == null || !total) return "";
  const used = Math.max(0, Math.min(100, Math.round(((total - disk.free) / total) * 100)));
  const failing = disk.health === "failing";
  const filling = used >= 85;
  const word = failing ? "Failing" : filling ? "Filling up" : "";
  return `<div class="room${failing ? " failing" : ""}" role="img" aria-label="${used}% in use"><span style="width:${used}%"></span></div>${word ? `<div class="room-word${failing ? " failing" : ""}">${word}</div>` : ""}`;
}

function disksCard(storage, failing) {
  const disks = (storage.disks || []).map((row) => row.disk ? { ...row.disk, holds: row.holds } : row);
  const total = disks.reduce((sum, disk) => sum + (disk.size || 0), 0);
  const size = keelSize(total).split(" ");
  const rows = disks.map((disk) => {
    const name = disk.model || disk.name || "Disk";
    const extra = disk.transport === "usb" ? " · USB" : "";
    const shown = keelSize(disk.size);
    const failingDisk = disk.health === "failing";
    return `<div class="disk-row${failingDisk ? " failing" : ""}">
      <div class="name">${escapeHtml(name)}</div>
      <div class="size">${escapeHtml(shown)}</div>
      <div class="holds">${escapeHtml(disk.holds || "")}${escapeHtml(extra)}</div>
      ${roomBar(disk)}
    </div>`;
  }).join("");
  const headline = disks.length
    ? `<p class="display">${escapeHtml(size[0])} <span>${escapeHtml(size[1] || "")}</span></p>`
    : `<p>No disks yet.</p>`;
  return `<article class="card home-card">
    <div class="home-card-head">
      <h2 class="home-title"><span class="home-icon" aria-hidden="true">${homeIcon("Disks")}</span><span>Disks</span> ${keelHelp("The big number is how much all the disks in this computer hold. Each bar fills as its disk fills up.")}</h2>
      <span class="pill ${failing ? "bad" : ""}">${failing ? "Needs a look" : (disks.length === 1 ? "1 disk" : `${disks.length} disks`)}</span>
    </div>
    ${headline}
    <div class="disk-list">${rows}</div>
  </article>`;
}

function showReach(page) {
  const box = $("reach");
  if (!page) {
    box.hidden = true;
    return;
  }
  box.hidden = false;
  $("reach-address").textContent = page;
  const copy = $("reach-copy");
  copy.onclick = async () => {
    try {
      await navigator.clipboard.writeText(page);
      copy.textContent = "Copied";
    } catch {
      copy.textContent = "Select the address and copy it";
    }
    window.setTimeout(() => { copy.textContent = "Copy"; }, 1600);
  };
}

function showNotice(lines, href, failing) {
  const box = $("notice");
  if (!lines.length) {
    box.hidden = true;
    box.replaceChildren();
    return;
  }
  const title = failing ? "Needs fixing" : lines.length === 1 ? "1 thing to look at" : `${lines.length} things to look at`;
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

// A copy of this computer on a Keel disk: when it was made, in words.
function spareWhen(text) {
  const when = new Date(text);
  if (!text || Number.isNaN(when.getTime())) return "an earlier day";
  return when.toLocaleString([], { weekday: "long", day: "numeric", month: "long", hour: "2-digit", minute: "2-digit" });
}

// Brings the copy back, then waits for Keel to restart and reloads.
async function bringBack(status, button) {
  button.disabled = true;
  status.textContent = "Bringing this computer back.";
  try {
    const res = await fetch("/api/spare/restore", { method: "POST", credentials: "same-origin" });
    const data = await res.json().catch(() => ({}));
    if (res.status === 401) throw new Error("Sign in first, then press it again.");
    if (!res.ok || !data.ok) throw new Error(data.message || "Keel could not bring this computer back.");
    status.textContent = data.message;
  } catch (err) {
    status.textContent = String(err.message || err);
    button.disabled = false;
    return;
  }
  const started = Date.now();
  const wait = async () => {
    await new Promise((done) => setTimeout(done, 2000));
    try {
      const health = await fetch("/api/health", { cache: "no-store" }).then((r) => r.json());
      if (Date.now() - started > 6000 && health.uptime_ms < Date.now() - started) {
        window.location.reload();
        return;
      }
    } catch { /* Keel is restarting */ }
    if (Date.now() - started < 120000) wait();
    else window.location.reload();
  };
  wait();
}

async function showSpare() {
  const box = $("spare-offer");
  if (!box) return;
  const spare = await fetch("/api/spare", { credentials: "same-origin" }).then((r) => r.json()).catch(() => ({}));
  if (!spare.found || !spare.fresh) {
    box.hidden = true;
    return;
  }
  $("spare-offer-text").textContent = `A Keel copy from ${spareWhen(spare.when)} is on ${spare.disk}. Restore its settings and app data; files stay on their disks.`;
  box.hidden = false;
  const go = $("spare-offer-go");
  if (!go.dataset.ready) {
    go.dataset.ready = "1";
    go.addEventListener("click", () => bringBack($("spare-offer-status"), go));
  }
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
  rows.innerHTML = choices.map((app) => `<label class="chip-check">
      <input type="checkbox" name="app" value="${escapeHtml(app.id)}">
      <span>${escapeHtml(app.role)}</span>
      ${keelHelp(app.why, `About ${app.role}`)}
    </label>`).join("");
  // Which version Keel suggests goes into that app's ? bubble.
  fetch("/api/versions", { credentials: "same-origin" })
    .then((res) => (res.ok ? res.json() : []))
    .then((list) => {
      const forApp = { video: "jitsi", photos: "immich" };
      for (const v of list) {
        const input = rows.querySelector(`input[value="${forApp[v.app] || v.app}"]`);
        const help = input && input.closest("label").querySelector("button.help");
        if (!help || help.dataset.version) continue;
        const name = v.suggested === "light" ? v.light : v.full;
        help.dataset.version = "1";
        help.dataset.tip += ` Keel suggests the ${v.suggested} version here (${name}). Settings can change it.`;
      }
    })
    .catch(() => {});
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
  keelShowHealth(issues.data, health.data.version);
  $("rest").innerHTML = `<a href="/sleep.html">Let this computer rest</a> when you are done.`;

  // The notice counts what Repair counts, so Home, Repair and the sidebar agree.
  const items = issues.data.items || [];
  const red = items.filter((item) => item.severity === "red");
  const lines = (red.length ? red : items).map((item) => `${item.title}.`);
  const onlyDisk = !red.length && items.length && items.every((item) => item.id === "no-pool" || item.id === "no-share");
  const fix = afterSignIn(onlyDisk ? "/pools.html" : "/repair.html", signedIn);
  showNotice(lines, fix, red.length > 0);
  showReach(s.page);
  showAppChoice(apps.data);
  showSpare();

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

load().catch(() => {
  showNotice(["Keel did not answer. Check that the Keel computer is on, then reload this page."], "/repair.html", true);
});

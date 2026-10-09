const $ = (id) => document.getElementById(id);
const NAMES_KEY = "keel-their-names";
let lastPlaces = [];
let lastListing = null;

function showTheirNames() {
  return localStorage.getItem(NAMES_KEY) === "1";
}

function displayName(plain, their) {
  const t = String(their || "").trim();
  if (showTheirNames() && t) return t;
  return plain;
}

function where(kind) {
  return kind === "foreign" ? "another computer" : "this machine";
}

function looksLikeWebsite(source) {
  const s = String(source || "").trim();
  if (!s) return false;
  if (s.includes("\\") || s.includes(":/") || s.startsWith("/")) return false;
  return true;
}

async function load() {
  const pools = await fetch("/api/pools").then((r) => r.json());
  $("rows").innerHTML = pools
    .map(
      (p) =>
        `<tr><td><code>${p.name}</code></td><td>${where(p.kind)}</td><td>${p.source || "—"}</td></tr>`
    )
    .join("") || `<tr><td colspan="3">No pool yet. Add one above.</td></tr>`;
  await renderRecents();
}

async function renderRecents() {
  const saved = await fetch("/api/look/saved").then((r) => r.json()).catch(() => ({ hosts: [] }));
  const hosts = saved.hosts || [];
  const box = $("recents");
  if (!hosts.length) {
    box.hidden = true;
    box.innerHTML = "";
    return;
  }
  box.hidden = false;
  box.innerHTML =
    `<p class="meta">Remembered — click, then sign in again (password is not kept)</p>` +
    hosts
      .map(
        (h) =>
          `<div class="recent-row">
            <button type="button" class="use" data-use="${encodeURIComponent(h.host)}" data-user="${encodeURIComponent(h.username || "")}" data-realm="${encodeURIComponent(h.realm || "pam")}">${escapeHtml(h.host)}</button>
            <button type="button" data-forget="${encodeURIComponent(h.host)}">Delete</button>
          </div>`
      )
      .join("");
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
}

$("form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const fd = new FormData(e.target);
  const res = await fetch("/api/pools", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      name: fd.get("name"),
      kind: fd.get("kind"),
      source: fd.get("source") || "",
    }),
  });
  if (!res.ok) {
    $("status").textContent = await res.text();
    await load();
    return;
  }
  const src = (fd.get("source") || "").trim();
  $("status").textContent = looksLikeWebsite(src)
    ? "saved as a plan — that address looks like a website, not a folder. Sign in on the left; only press Use on a row that already shows a folder path."
    : "saved as a plan. This PC is not mounting it. Files still live on the other computer.";
  await load();
});

$("lookform").addEventListener("submit", async (e) => {
  e.preventDefault();
  const host = $("host").value;
  $("status").textContent = "looking…";
  $("found").hidden = false;
  const res = await fetch("/api/look", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ host }),
  });
  if (!res.ok) {
    $("status").textContent = await res.text();
    $("found").innerHTML = "";
    return;
  }
  const report = await res.json();
  $("status").textContent = `looked at ${report.host}`;
  await renderRecents();
  const open = report.hits.filter((h) => h.open);
  const closed = report.hits.filter((h) => !h.open);
  $("found").innerHTML =
    `<p class="prose">${report.why}</p>` +
    (open.length
      ? open
          .map(
            (h) =>
              `<article class="opp"><h3>${escapeHtml(h.name)}</h3><p>${escapeHtml(h.hint)}</p>${
                h.fill
                  ? `<button type="button" data-fill="${encodeURIComponent(h.fill)}">Use this folder</button>`
                  : ""
              }</article>`
          )
          .join("")
      : `<p class="prose">Nothing answered. Wrong name, the computer is off, or a firewall is in the way. Try the address you already use in the browser.</p>`) +
    (closed.length
      ? `<p class="meta">Nothing else answered.</p>`
      : "");
});

$("signinform").addEventListener("submit", async (e) => {
  e.preventDefault();
  $("status").textContent = "signing in…";
  $("places").hidden = false;
  const res = await fetch("/api/look/login", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      host: $("host").value,
      username: $("user").value,
      password: $("pass").value,
      realm: $("realm").value,
      insecure: $("insecure").checked,
    }),
  });
  if (!res.ok) {
    $("status").textContent = await res.text();
    $("places").innerHTML = "";
    return;
  }
  $("pass").value = "";
  const report = await res.json();
  $("status").textContent = `signed in to ${report.product} as ${report.who}`;
  await renderRecents();
  $("signinwhy").textContent = report.why;
  renderPlaces(report.places);
  $("filesform").hidden = false;
  $("filesinsecure").checked = $("insecure").checked;
});

function useAddress(fill, name) {
  const src = document.querySelector('input[name="source"]');
  src.value = fill;
  document.querySelector('select[name="kind"]').value = "foreign";
  const nameEl = document.querySelector('input[name="name"]');
  if (nameEl && !nameEl.value && name) nameEl.value = name;
  src.focus();
}

$("recents").addEventListener("click", async (e) => {
  const forget = e.target.dataset && e.target.dataset.forget;
  if (forget) {
    const host = decodeURIComponent(forget);
    if (!confirm(`Forget ${host}?`)) return;
    await fetch("/api/look/forget", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ host }),
    });
    await renderRecents();
    return;
  }
  const use = e.target.dataset && e.target.dataset.use;
  if (!use) return;
  $("host").value = decodeURIComponent(use);
  const user = e.target.dataset.user ? decodeURIComponent(e.target.dataset.user) : "";
  const realm = e.target.dataset.realm ? decodeURIComponent(e.target.dataset.realm) : "pam";
  if (user) $("user").value = user;
  if (realm) $("realm").value = realm;
  $("host").focus();
});

$("found").addEventListener("click", (e) => {
  const raw = e.target.dataset && e.target.dataset.fill;
  if (!raw) return;
  useAddress(decodeURIComponent(raw), "");
});

$("places").addEventListener("click", (e) => {
  const open = e.target.dataset && e.target.dataset.open;
  if (open) {
    window.open(decodeURIComponent(open), "_blank", "noopener");
    return;
  }
  const browse = e.target.dataset && e.target.dataset.browse;
  if (browse) {
    startBrowse(decodeURIComponent(browse));
    return;
  }
  const raw = e.target.dataset && e.target.dataset.fill;
  if (!raw) return;
  const name = e.target.dataset.name ? decodeURIComponent(e.target.dataset.name) : "";
  useAddress(decodeURIComponent(raw), name);
});

function proposalLabel(p) {
  if (p === "use") return "Use this";
  if (p === "list") return "You can list this";
  if (p === "leave") return "Leave this";
  return "";
}

function renderPlaces(places) {
  lastPlaces = places || [];
  $("places").hidden = false;
  $("places").innerHTML =
    (lastPlaces)
      .map((p) => {
        const shown = displayName(p.title, p.their_name);
        const btns = [];
        if (p.proposal !== "leave" && p.address) {
          btns.push(
            `<button type="button" data-fill="${encodeURIComponent(p.address)}" data-name="${encodeURIComponent(shown)}">Use this folder</button>`
          );
        }
        if (p.open_url) {
          btns.push(`<button type="button" data-open="${encodeURIComponent(p.open_url)}">Open in browser</button>`);
        }
        if (p.browse) {
          btns.push(`<button type="button" data-browse="${encodeURIComponent(p.browse)}">List files</button>`);
        }
        const tag = proposalLabel(p.proposal);
        return `<article class="opp ${escapeHtml(p.proposal || "")}"><h3>${escapeHtml(shown)}${tag ? ` · ${tag}` : ""}</h3><p>${escapeHtml(p.why)}</p>${btns.join(" ")}</article>`;
      })
      .join("") || `<p class="prose">Signed in. No shared folder a PC can open. Family files may still be in the files app.</p>`;
}

function viaOf(browse) {
  const i = browse.indexOf(":");
  if (i < 0) return { via: browse, target: browse };
  return { via: browse.slice(0, i), target: browse };
}

async function startBrowse(browse) {
  const { via, target } = viaOf(browse);
  $("listing").hidden = false;
  if (via === "storage") {
    $("filesform").hidden = true;
    $("status").textContent = "listing…";
    await postBrowse({ via, target, path: "", username: "", password: "", insecure: $("insecure").checked });
    return;
  }
  $("filesform").hidden = false;
  $("filesform").dataset.via = via;
  $("filesform").dataset.target = target;
  if (via === "webdav" && target.startsWith("webdav:")) {
    $("filesurl").value = target.slice("webdav:".length);
  }
  $("filesinsecure").checked = $("insecure").checked;
  $("listing").innerHTML = `<p class="prose">That row needs the files-app (or folder) name and password. Not the Proxmox one.</p>`;
  $("filesuser").focus();
}

async function postBrowse(body) {
  const res = await fetch("/api/look/browse", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    $("status").textContent = await res.text();
    $("listing").innerHTML = "";
    $("listing").hidden = false;
    return;
  }
  const report = await res.json();
  $("status").textContent = "listed files";
  renderListing(report);
}

function bytes(n) {
  if (!n) return "";
  const u = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < u.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v < 10 && i > 0 ? v.toFixed(1) : Math.round(v)} ${u[i]}`;
}

function renderListing(report) {
  lastListing = report;
  $("listing").hidden = false;
  const rows = (report.entries || [])
    .map((e) => {
      const shown = displayName(e.name, e.their_name);
      const btns = [];
      if (e.proposal !== "leave" && e.address) {
        btns.push(
          `<button type="button" data-fill="${encodeURIComponent(e.address)}" data-name="${encodeURIComponent(shown)}">Use this folder</button>`
        );
      }
      if (e.open_url) {
        btns.push(`<button type="button" data-open="${encodeURIComponent(e.open_url)}">Open</button>`);
      }
      if (e.browse) {
        btns.push(`<button type="button" data-browse="${encodeURIComponent(e.browse)}">List</button>`);
      }
      const sz = e.size ? ` · ${bytes(e.size)}` : "";
      const tag = proposalLabel(e.proposal);
      return `<article class="opp ${escapeHtml(e.proposal || "")}"><h3>${escapeHtml(shown)}${tag ? ` · ${tag}` : ""}${sz}</h3><p>${escapeHtml(e.why)}</p>${btns.join(" ")}</article>`;
    })
    .join("");
  $("listing").innerHTML =
    `<p class="prose">${escapeHtml(report.why)}</p>` +
    (rows || `<p class="prose">Nothing to list.</p>`);
}

$("filesform").addEventListener("submit", async (e) => {
  e.preventDefault();
  const typed = ($("filesurl").value || "").trim();
  let via = $("filesform").dataset.via || "webdav";
  let target = $("filesform").dataset.target || "";
  if (typed) {
    via = typed.startsWith("\\\\") ? "smb" : "webdav";
    target = via === "smb" ? `smb:${typed}` : `webdav:${typed.replace(/\/$/, "")}`;
  }
  $("status").textContent = "listing…";
  await postBrowse({
    via,
    target,
    path: "",
    username: $("filesuser").value,
    password: $("filespass").value,
    insecure: $("filesinsecure").checked,
  });
  $("filespass").value = "";
});

$("listing").addEventListener("click", (e) => {
  const open = e.target.dataset && e.target.dataset.open;
  if (open) {
    window.open(decodeURIComponent(open), "_blank", "noopener");
    return;
  }
  const browse = e.target.dataset && e.target.dataset.browse;
  if (browse) {
    startBrowse(decodeURIComponent(browse));
    return;
  }
  const raw = e.target.dataset && e.target.dataset.fill;
  if (!raw) return;
  const name = e.target.dataset.name ? decodeURIComponent(e.target.dataset.name) : "";
  useAddress(decodeURIComponent(raw), name);
});

async function restoreSession() {
  const s = await fetch("/api/look/session").then((r) => r.json()).catch(() => ({ signed_in: false }));
  if (!s.signed_in) return;
  $("status").textContent = `still signed in to ${s.product} as ${s.who}`;
  const files = await fetch("/api/look/files");
  if (!files.ok) return;
  const report = await files.json();
  $("signinwhy").textContent = report.why;
  renderPlaces(
    (report.entries || []).map((e) => ({
      title: e.name,
      kind: e.kind,
      address: e.address,
      why: e.why,
      browse: e.browse,
      open_url: e.open_url,
      proposal: e.proposal,
      their_name: e.their_name,
    }))
  );
  $("filesform").hidden = false;
}

$("theirnames").checked = showTheirNames();
$("theirnames").addEventListener("change", () => {
  localStorage.setItem(NAMES_KEY, $("theirnames").checked ? "1" : "0");
  if (lastPlaces.length) renderPlaces(lastPlaces);
  if (lastListing) renderListing(lastListing);
});

load().then(restoreSession).catch((err) => {
  $("status").textContent = String(err);
});

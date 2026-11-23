const $ = (id) => document.getElementById(id);

function escapeHtml(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
}

let home = { folders: [], sample: false, message: "" };
let share = "";
let path = "";
let listing = null;
let pending = null;

function sizeOf(entry) {
  if (entry.kind !== "file") return "";
  const n = entry.size || 0;
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${Math.max(1, Math.round(n / 1024))} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(n < 10 * 1024 * 1024 ? 1 : 0)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function kindWord(kind) {
  if (kind === "folder") return "Folder";
  if (kind === "file") return "File";
  return "Link";
}

function joinPath(dir, name) {
  return dir ? `${dir}/${name}` : name;
}

function inside(folder, dest) {
  return folder && (dest === folder || dest.startsWith(`${folder}/`));
}

async function readJson(res) {
  const text = await res.text();
  try {
    return JSON.parse(text);
  } catch {
    return {
      ok: res.ok,
      message: res.status === 401 ? "Sign in first (bottom of the sidebar)." : text,
    };
  }
}

function currentFolder() {
  return home.folders.find((folder) => folder.name === share) || null;
}

function renderFolders() {
  $("folders").innerHTML = home.folders
    .map((folder) => {
      const note = folder.read_only ? " · copy only" : "";
      return `<button type="button" class="roster-item${folder.name === share ? " active" : ""}" data-share="${escapeHtml(folder.name)}">${escapeHtml(folder.name)}${note}</button>`;
    })
    .join("");
}

function renderCrumb() {
  const bits = [];
  if (share) {
    bits.push(`<button type="button" data-crumb="">${escapeHtml(share)}</button>`);
  }
  if (path) {
    const parts = path.split("/");
    parts.forEach((part, index) => {
      const at = parts.slice(0, index + 1).join("/");
      const current = index === parts.length - 1;
      bits.push(
        current
          ? `<button type="button" data-crumb="${escapeHtml(at)}" aria-current="page">${escapeHtml(part)}</button>`
          : `<button type="button" data-crumb="${escapeHtml(at)}">${escapeHtml(part)}</button>`,
      );
    });
  }
  $("crumb").innerHTML = bits.join(" / ");
}

function renderPending() {
  const bar = $("pending");
  if (!pending) {
    bar.hidden = true;
    return;
  }
  bar.hidden = false;
  const folder = currentFolder();
  const intoSelf = pending.kind === "folder" && pending.share === share && inside(pending.path, path);
  const readOnly = Boolean(folder && folder.read_only);
  $("put").disabled = readOnly || intoSelf;
  $("put").textContent = pending.op === "move" ? "Move it here" : "Copy it here";
  if (readOnly) {
    $("pendingtext").textContent = "This folder only keeps copies. Open Documents, Work, or Media.";
  } else if (intoSelf) {
    $("pendingtext").textContent = "Open a folder that is not inside this one.";
  } else {
    $("pendingtext").textContent = `Open the folder where ${pending.name} should go, then press Put it here.`;
  }
}

// Every picture in Files shows a small copy. Pictures in the Photos library
// use the copies Photos already keeps; others get their own, kept by Keel.
const PICTURE = /\.(jpe?g|png|gif|webp|heic|heif)$/i;

function pictureLinks(entry) {
  if (entry.kind !== "file" || !PICTURE.test(entry.name)) return null;
  const rel = joinPath(path, entry.name);
  const v = `${Number(entry.modified_ms || 0).toString(16)}${Number(entry.size || 0).toString(16)}`;
  const top = rel.split("/")[0];
  const inPhotos = share === "Media" && (top === "Photos" || top === "Pictures");
  const link = (size) => inPhotos
    ? `/api/photos/picture?${new URLSearchParams({ path: rel, size, v })}`
    : `/api/folders/picture?${new URLSearchParams({ share, path: rel, size, v })}`;
  return {
    name: entry.name,
    grid: link("grid"),
    small: link("small"),
    screen: link("screen"),
    download: `/api/folders/file?${new URLSearchParams({ share, path: rel })}`,
  };
}

// The pictures in the open folder, for the viewer's Previous and Next.
let folderPictures = [];

// A long folder would keep every picture scrolled past in memory. Pictures
// well away from view let go of their image and get it back when near.
let thumbWatch = null;

function watchThumbs() {
  if (thumbWatch) thumbWatch.disconnect();
  thumbWatch = null;
  const thumbs = $("rows").querySelectorAll("img.file-thumb");
  for (const img of thumbs) img.addEventListener("error", () => img.remove(), { once: true });
  if (!thumbs.length || !("IntersectionObserver" in window)) return;
  let root = $("rows").parentElement;
  while (root && root !== document.body && !/(auto|scroll)/.test(getComputedStyle(root).overflowY)) root = root.parentElement;
  thumbWatch = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      const img = entry.target;
      if (entry.isIntersecting) {
        if (img.dataset.src) {
          img.src = img.dataset.src;
          delete img.dataset.src;
        }
      } else if (img.getAttribute("src")) {
        img.dataset.src = img.getAttribute("src");
        img.removeAttribute("src");
      }
    }
  }, { root: root === document.body ? null : root, rootMargin: "800px 0px" });
  for (const img of thumbs) thumbWatch.observe(img);
}

function renderListing() {
  const folder = currentFolder();
  const readOnly = Boolean(folder && folder.read_only) || Boolean(listing && listing.read_only);
  const none = !(home && home.folders && home.folders.length);
  $("newfolder").hidden = readOnly || none;
  $("newsection").hidden = none;
  if (!listing) {
    $("rows").innerHTML = none
      ? `<tr><td colspan="4"><div class="empty-state"><p>No folders yet. Give a disk the job of keeping files, and Documents, Work and Media appear here.</p><a class="button" href="/pools.html">Choose a disk</a></div></td></tr>`
      : `<tr><td colspan="4">Choose a folder on the left.</td></tr>`;
    renderCrumb();
    renderPending();
    return;
  }
  folderPictures = [];
  const rows = listing.entries.map((entry) => {
    const picture = pictureLinks(entry);
    let name = escapeHtml(entry.name);
    if (entry.kind === "folder") {
      name = `<button type="button" class="texty" data-open="${escapeHtml(entry.name)}">${escapeHtml(entry.name)}</button>`;
    } else if (picture) {
      const index = folderPictures.push(picture) - 1;
      name = `<button type="button" class="texty file-picture" data-view="${index}" aria-label="Look at ${escapeHtml(entry.name)}">`
        + `<img class="file-thumb" alt="" loading="lazy" decoding="async" src="${escapeHtml(picture.grid)}">${escapeHtml(entry.name)}</button>`;
    }
    const canMove = !readOnly && entry.kind !== "link";
    const canCopy = entry.kind !== "link";
    const download = entry.kind === "file"
      ? `<a class="button" href="/api/folders/file?${new URLSearchParams({ share, path: joinPath(path, entry.name) })}" download="${escapeHtml(entry.name)}">Download</a>`
      : "";
    const actions = [
      download,
      canMove ? `<button type="button" data-op="move" data-name="${escapeHtml(entry.name)}" data-kind="${entry.kind}">Move</button>` : "",
      canCopy ? `<button type="button" data-op="copy" data-name="${escapeHtml(entry.name)}" data-kind="${entry.kind}">Copy</button>` : "",
    ].join("");
    return `<tr><td>${name}</td><td>${kindWord(entry.kind)}</td><td>${sizeOf(entry)}</td><td class="row-actions">${actions}</td></tr>`;
  });
  $("rows").innerHTML = rows.join("") || `<tr><td colspan="4">Empty folder.</td></tr>`;
  watchThumbs();
  renderCrumb();
  renderPending();
}

async function openFolder(nextShare, nextPath) {
  share = nextShare;
  path = nextPath || "";
  $("status").textContent = "Opening…";
  const query = new URLSearchParams({ share, path });
  const res = await fetch(`/api/folders/list?${query}`, { credentials: "same-origin" });
  const data = await readJson(res);
  if (!res.ok) {
    listing = null;
    $("status").textContent = data.message || "Keel could not open that folder.";
    renderFolders();
    renderListing();
    return;
  }
  listing = data;
  $("status").textContent = "";
  renderFolders();
  renderListing();
}

async function loadHome() {
  const res = await fetch("/api/folders", { credentials: "same-origin" });
  home = await readJson(res);
  if (!res.ok) {
    home = { folders: [], message: home.message || "Sign in first (bottom of the sidebar)." };
  }
  $("lede").textContent = home.sample ? home.message : "The house's shared folders.";
  if (!share || !home.folders.some((folder) => folder.name === share)) {
    share = (home.folders.find((folder) => !folder.read_only) || home.folders[0] || {}).name || "";
    path = "";
  }
  renderFolders();
  if (share) await openFolder(share, path);
  else renderListing();
}

async function loadShares() {
  const [pools, shares] = await Promise.all([
    fetch("/api/pools", { credentials: "same-origin" }).then((r) => r.json()),
    fetch("/api/shares", { credentials: "same-origin" }).then((r) => r.json()),
  ]);
  $("pool").innerHTML = pools
    .map((p) => `<option value="${escapeHtml(p.name)}">${escapeHtml(p.name)} (${p.kind === "foreign" ? "another computer" : "this machine"})</option>`)
    .join("") || `<option value="">Choose disks first</option>`;
  const lines = shares.map((item) => {
    const addr = (item.lan && item.lan[0]) || "";
    const overlay = (item.overlay && item.overlay[0]) || "";
    const bits = [item.name];
    if (addr) bits.push(addr);
    if (overlay) bits.push(`Away: ${overlay}`);
    if (!item.ready) bits.push("not ready");
    return bits.join(" — ");
  });
  const first = shares[0];
  if (first && first.username && first.password) {
    lines.push(`Name: ${first.username}  Password: ${first.password}`);
  } else if (first && first.username) {
    lines.push(`Name: ${first.username}`);
  }
  if (lines.length) {
    $("access").textContent = lines.join("\n");
    return;
  }
  let platform = document.documentElement.dataset.platform;
  if (!platform) {
    try {
      const sys = await fetch("/api/system", { credentials: "same-origin" }).then((r) => r.json());
      platform = sys.platform;
    } catch {
      platform = "";
    }
  }
  $("access").textContent = platform === "windows"
    ? "No name yet. This Windows copy cannot serve folders. The Keel computer can."
    : "No name yet. Add one, then open it from another computer.";
}

$("newsection").addEventListener("submit", async (event) => {
  event.preventDefault();
  const name = $("sectionname").value.trim();
  $("sectionstatus").textContent = "Adding…";
  const res = await fetch("/api/folders/section", {
    method: "POST",
    credentials: "same-origin",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ name }),
  });
  const data = await readJson(res);
  $("sectionstatus").textContent = data.message || "Keel could not add that section.";
  if (!data.ok) return;
  $("sectionname").value = "";
  await loadHome();
  await loadShares().catch(() => {});
});

$("folders").addEventListener("click", (event) => {
  const button = event.target.closest("button[data-share]");
  if (!button) return;
  openFolder(button.dataset.share, "").catch((err) => {
    $("status").textContent = String(err);
  });
});

$("crumb").addEventListener("click", (event) => {
  const button = event.target.closest("button[data-crumb]");
  if (!button || !share) return;
  openFolder(share, button.dataset.crumb).catch((err) => {
    $("status").textContent = String(err);
  });
});

// Looking at a picture: its screen copy, with Previous and Next for the other
// pictures in the folder. The small copy shows first, while the screen copy comes.
let viewing = -1;
let viewToken = 0;

function showPicture(index) {
  const picture = folderPictures[index];
  if (!picture) return;
  viewing = index;
  const token = ++viewToken;
  const image = $("file-image");
  image.alt = picture.name;
  image.src = picture.small;
  const screen = new Image();
  screen.onload = () => {
    if (token !== viewToken || $("file-viewer").hidden) return;
    image.src = picture.screen;
    // Get the next one ready while this one is looked at.
    const next = folderPictures[index + 1];
    if (next) new Image().src = next.screen;
  };
  screen.src = picture.screen;
  $("file-name").textContent = picture.name;
  $("file-download").href = picture.download;
  $("file-download").setAttribute("download", picture.name);
  $("file-count").textContent = `${index + 1} of ${folderPictures.length}`;
  $("file-prev").disabled = index <= 0;
  $("file-next").disabled = index >= folderPictures.length - 1;
  if ($("file-viewer").hidden) {
    $("file-viewer").hidden = false;
    $("file-back").focus();
  }
}

function closePicture() {
  viewToken += 1;
  $("file-viewer").hidden = true;
  $("file-image").removeAttribute("src");
  $("rows").querySelector(`button[data-view="${viewing}"]`)?.focus();
}

$("file-back").addEventListener("click", closePicture);
$("file-prev").addEventListener("click", () => showPicture(viewing - 1));
$("file-next").addEventListener("click", () => showPicture(viewing + 1));
document.addEventListener("keydown", (event) => {
  if ($("file-viewer").hidden) return;
  if (event.key === "Escape") closePicture();
  if (event.key === "ArrowLeft") showPicture(viewing - 1);
  if (event.key === "ArrowRight") showPicture(viewing + 1);
});

$("rows").addEventListener("click", (event) => {
  const view = event.target.closest("button[data-view]");
  if (view) {
    showPicture(Number(view.dataset.view));
    return;
  }
  const open = event.target.closest("button[data-open]");
  if (open) {
    openFolder(share, joinPath(path, open.dataset.open)).catch((err) => {
      $("status").textContent = String(err);
    });
    return;
  }
  const op = event.target.closest("button[data-op]");
  if (!op) return;
  pending = {
    op: op.dataset.op,
    share,
    path: joinPath(path, op.dataset.name),
    name: op.dataset.name,
    kind: op.dataset.kind,
  };
  renderPending();
});

$("newfolder").addEventListener("submit", async (event) => {
  event.preventDefault();
  const name = $("newname").value.trim();
  if (!name) {
    $("status").textContent = "Type a name for the folder.";
    return;
  }
  $("status").textContent = "Making folder…";
  const res = await fetch("/api/folders/mkdir", {
    method: "POST",
    credentials: "same-origin",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ share, path, name }),
  });
  const data = await readJson(res);
  $("status").textContent = data.message || "";
  if (data.ok) {
    $("newname").value = "";
    await openFolder(share, path);
  }
});

$("put").addEventListener("click", async () => {
  if (!pending) return;
  $("put").disabled = true;
  $("status").textContent = pending.op === "move" ? "Moving…" : "Copying…";
  const res = await fetch(`/api/folders/${pending.op}`, {
    method: "POST",
    credentials: "same-origin",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      from_share: pending.share,
      from_path: pending.path,
      to_share: share,
      to_path: path,
    }),
  });
  const data = await readJson(res);
  if (data.ok) {
    const message = data.message || "";
    pending = null;
    await openFolder(share, path);
    $("status").textContent = message;
  } else {
    $("status").textContent = data.message || "Keel could not finish that.";
    renderPending();
  }
});

$("cancel").addEventListener("click", () => {
  pending = null;
  $("status").textContent = "";
  renderPending();
});

$("form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const fd = new FormData(event.target);
  const res = await fetch("/api/shares", {
    method: "POST",
    credentials: "same-origin",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      name: fd.get("name"),
      pool: fd.get("pool"),
      path: fd.get("path") || "/",
    }),
  });
  $("addstatus").textContent = res.ok ? "added" : await res.text();
  await loadShares();
});

async function loadOwner() {
  const res = await fetch("/api/storage", { credentials: "same-origin" });
  const storage = await res.json();
  const owner = storage.owner;
  $("ownername").value = owner || "";
  $("ownername").readOnly = Boolean(owner);
  $("ownergo").textContent = owner ? "Change password" : "Use this name";
  $("ownerwhy").textContent = owner
    ? `Every folder opens with the name ${owner} and its password, at home and from Away.`
    : "Right now anyone on this house network can open the folders. Choose one name and password and only that opens them, also from Away.";
}

$("ownerform").addEventListener("submit", async (event) => {
  event.preventDefault();
  const password = $("ownerpass").value;
  if (password !== $("ownerpass2").value) {
    $("ownerstatus").textContent = "The two passwords are different.";
    return;
  }
  $("ownergo").disabled = true;
  $("ownerstatus").textContent = "Saving…";
  const res = await fetch("/api/storage/owner", {
    method: "POST",
    credentials: "same-origin",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ name: $("ownername").value.trim(), password }),
  });
  const data = await readJson(res);
  $("ownergo").disabled = false;
  $("ownerstatus").textContent = data.message || "";
  if (data.ok) {
    $("ownerpass").value = "";
    $("ownerpass2").value = "";
  }
  await Promise.all([loadShares(), loadOwner()]).catch(() => {});
});

loadHome().catch((err) => {
  $("status").textContent = String(err);
});
loadShares().catch((err) => {
  $("access").textContent = String(err);
});
loadOwner().catch((err) => {
  $("ownerwhy").textContent = String(err);
});

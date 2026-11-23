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

(() => {
  const el = (id) => document.getElementById(id);
  const esc = (s) =>
    String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));

  let view = null;
  let chosen = null;
  let busy = false;

  const size = (n) => (n ? keelSize(n) : "");

  function when(ms) {
    if (!ms) return "never";
    const d = new Date(ms);
    return d.toLocaleString([], { dateStyle: "medium", timeStyle: "short" });
  }

  function diskName(d) {
    const model = d.model || "Disk";
    const via = d.transport === "usb" ? " · USB" : d.transport === "nvme" ? " · NVMe" : "";
    return `<div class="disk-model">${esc(model)}</div><div class="disk-dev">${esc(d.name)}${via}</div>`;
  }

  function nextRole() {
    const free = view.roles.find((r) => !r.disk);
    return free ? free.id : "";
  }

  function room(d) {
    const total = d.fs_size || d.size || 0;
    if (d.free == null || !total) return "";
    const used = Math.max(0, Math.min(100, Math.round(((total - d.free) / total) * 100)));
    const failing = d.health === "failing";
    const word = failing ? "Failing" : used >= 85 ? "Filling up" : `${used}% in use`;
    return `<div class="room${failing ? " failing" : ""}" role="img" aria-label="${used}% in use"><span style="width:${used}%"></span></div><span class="meta">${word}</span>`;
  }

  function renderDisks() {
    const rows = view.disks.map((d, i) => {
      const offers = d.offers || [];
      const buttons = [];
      if (offers.includes("files")) buttons.push(`<button type="button" data-keep="files" data-index="${i}">These are the Files</button>`);
      if (offers.includes("software")) buttons.push(`<button type="button" data-keep="software" data-index="${i}">These are Software and Inbox</button>`);
      if (offers.includes("everyday")) buttons.push(`<button type="button" data-keep="everyday" data-index="${i}">Use this disk</button>`);
      if (d.can_use) buttons.push(`<button type="button" data-use="${i}">Give it a job…</button>`);
      return `<tr class="${d.health === "failing" ? "failing" : ""}"><td>${diskName(d)}</td><td class="disk-size">${size(d.size)}${room(d)}</td><td class="cell-wide">${esc(d.holds)}</td><td><div class="row-actions">${buttons.join(" ")}</div></td></tr>`;
    });
    el("diskrows").innerHTML = rows.join("") || `<tr><td colspan="4">No disks found.</td></tr>`;
  }

  function renderCopies() {
    const c = view.copies;
    const hasCopyDisk = view.roles.some((r) => (r.id === "copy" || r.id === "extra") && r.disk);
    let text;
    if (view.copy_running) text = "Copying now. The first copy can take hours.";
    else if (!hasCopyDisk) text = "Needs a second disk with the Nightly copy job.";
    else if (!view.can_copy) text = "Waiting for the Files disk and the copy disk to be connected.";
    else if (!c.last_run_ms) text = "The first copy starts in a minute or two.";
    else text = `Last copy ${when(c.last_run_ms)}. ${c.message}`;
    el("copystatus").textContent = text;
    el("copynow").hidden = !view.can_copy;
    el("copynow").disabled = view.copy_running;
    renderComputer();
  }

  function renderComputer() {
    const c = view.computer_copy || {};
    let text;
    if (!view.can_copy_computer) {
      text = "Needs a disk with the Nightly copy job.";
    } else if (c.message) {
      text = c.message;
    } else {
      text = "Saved every night onto the Nightly copy disk.";
    }
    el("computerstatus").textContent = text;
    el("computersavego").disabled = !view.can_copy_computer;
    renderSpare();
  }

  // Bringing a copy back replaces this computer's settings, so it asks once.
  let spare = {};
  async function renderSpare() {
    spare = await fetch("/api/spare", { credentials: "same-origin" }).then((r) => r.json()).catch(() => ({}));
    el("spare").hidden = !spare.found;
    if (!spare.found || el("sparego").dataset.asked) return;
    el("sparetext").textContent = `Last copy on ${spare.disk}: ${spareWhen(spare.when)}.`;
  }

  function showUse(index) {
    chosen = view.disks[index];
    el("usewhich").textContent = `${chosen.model || "Disk"} (${chosen.name}), ${size(chosen.size)}. ${chosen.holds}`;
    el("userole").innerHTML = view.roles
      .map((r) => `<option value="${r.id}" ${r.disk ? "disabled" : ""}>${esc(r.title)}${r.disk ? ` (on ${esc(r.disk)})` : ""}</option>`)
      .join("");
    el("userole").value = nextRole();
    el("usename").innerHTML = `<code>${esc(chosen.name)}</code>`;
    el("useconfirm").value = "";
    describeRole();
    el("useform").hidden = false;
    el("useconfirm").focus();
  }

  function describeRole() {
    const role = view.roles.find((r) => r.id === el("userole").value);
    el("userolewhat").textContent = role ? role.what : "Every job already has a disk.";
    checkConfirm();
  }

  function checkConfirm() {
    el("usego").disabled = busy || !chosen || !el("userole").value || el("useconfirm").value.trim() !== chosen.name;
  }

  async function load() {
    const res = await fetch("/api/storage", { credentials: "same-origin" });
    if (!res.ok) throw new Error(await res.text());
    view = await res.json();
    renderDisks();
    renderCopies();
  }

  async function post(url, body) {
    const res = await fetch(url, {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body || {}),
    });
    if (res.status === 401) return { ok: false, message: "Sign in first (bottom of the sidebar)." };
    const text = await res.text();
    try {
      return JSON.parse(text);
    } catch {
      return { ok: false, message: text };
    }
  }

  el("diskrows").addEventListener("click", async (e) => {
    const keep = e.target.closest("button[data-keep]");
    if (keep) {
      const disk = view.disks[Number(keep.dataset.index)];
      keep.disabled = true;
      const reply = await post("/api/storage/keep", { path: disk.path, kind: keep.dataset.keep });
      el("diskstatus").textContent = reply.message || (reply.ok ? "Done." : "That did not work.");
      await load().catch(() => {});
      return;
    }
    const button = e.target.closest("button[data-use]");
    if (button) showUse(Number(button.dataset.use));
  });
  el("userole").addEventListener("change", describeRole);
  el("useconfirm").addEventListener("input", checkConfirm);
  el("usecancel").addEventListener("click", () => {
    el("useform").hidden = true;
    chosen = null;
  });

  el("useform").addEventListener("submit", async (e) => {
    e.preventDefault();
    if (!chosen) return;
    busy = true;
    checkConfirm();
    el("diskstatus").textContent = `Erasing and preparing ${chosen.name}. This takes about a minute…`;
    const reply = await post("/api/storage/prepare", {
      path: chosen.path,
      role: el("userole").value,
      confirm: el("useconfirm").value,
    });
    busy = false;
    el("diskstatus").textContent = reply.message || (reply.ok ? "Done." : "That did not work.");
    if (reply.ok) {
      el("useform").hidden = true;
      chosen = null;
    }
    await load().catch(() => {});
    checkConfirm();
  });

  el("sleeptry").addEventListener("click", async () => {
    el("sleeptry").disabled = true;
    el("sleepstatus").textContent = "Asking one disk. This can take a minute the first time.";
    const reply = await post("/api/storage/sleep-try");
    el("sleepstatus").textContent = reply.message || "That try did not finish.";
    el("sleeptry").disabled = false;
  });

  el("copynow").addEventListener("click", async () => {
    el("copynow").disabled = true;
    const reply = await post("/api/storage/copy-now");
    el("copystatus").textContent = reply.message;
    setTimeout(() => load().catch(() => {}), 1500);
  });

  el("sparego").addEventListener("click", () => {
    const button = el("sparego");
    if (!button.dataset.asked) {
      button.dataset.asked = "1";
      el("sparetext").textContent = `This replaces the settings on this computer with the copy from ${spareWhen(spare.when)}: people, the name you own, disk jobs, and the apps' data. Running apps stop. Press again to bring it back.`;
      button.textContent = "Bring it back";
      return;
    }
    bringBack(el("sparetext"), button);
  });

  el("computersavego").addEventListener("click", async () => {
    el("computersavego").disabled = true;
    el("computerstatus").textContent = "Copying this computer.";
    const reply = await post("/api/storage/copy-computer");
    el("computerstatus").textContent = reply.message;
    el("computersavego").disabled = !view.can_copy_computer;
    await load().catch(() => {});
  });

  load().catch((err) => {
    el("diskstatus").textContent = String(err);
  });
  setInterval(() => {
    if (el("useform").hidden && !busy) load().catch(() => {});
  }, 10000);
})();

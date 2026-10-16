(() => {
  const el = (id) => document.getElementById(id);
  const esc = (s) =>
    String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));

  let view = null;
  let chosen = null;
  let busy = false;

  function size(n) {
    if (!n) return "";
    const tb = n / 1e12;
    return tb >= 1 ? `${tb.toFixed(1)} TB` : `${Math.round(n / 1e9)} GB`;
  }

  function when(ms) {
    if (!ms) return "never";
    const d = new Date(ms);
    return d.toLocaleString([], { dateStyle: "medium", timeStyle: "short" });
  }

  function diskName(d) {
    const model = d.model || "Disk";
    const via = d.transport === "usb" ? " · USB" : d.transport === "nvme" ? " · NVMe" : "";
    return `${esc(model)} <span class="meta">${esc(d.name)}${via}</span>`;
  }

  function nextRole() {
    const free = view.roles.find((r) => !r.disk);
    return free ? free.id : "";
  }

  function room(d) {
    const total = d.fs_size || d.size || 0;
    if (d.free == null || !total) return "";
    const left = Math.max(0, Math.min(100, Math.round((d.free / total) * 100)));
    const failing = d.health === "failing";
    const word = failing ? "Failing" : left < 15 ? "Filling up" : `${left}% free`;
    return `<div class="room${failing ? " failing" : ""}"><span style="width:${left}%"></span></div><span class="meta">${word}</span>`;
  }

  function renderDisks() {
    const rows = view.disks.map((d, i) => {
      const action = d.can_use ? `<button type="button" data-use="${i}">Use…</button>` : "";
      return `<tr class="${d.health === "failing" ? "failing" : ""}"><td>${diskName(d)}</td><td class="disk-size">${size(d.size)}${room(d)}</td><td>${esc(d.holds)}</td><td>${action}</td></tr>`;
    });
    el("diskrows").innerHTML = rows.join("") || `<tr><td colspan="4">No disks found.</td></tr>`;
  }

  function renderCopies() {
    const c = view.copies;
    const hasCopyDisk = view.roles.some((r) => (r.id === "copy" || r.id === "extra") && r.disk);
    let text;
    if (view.copy_running) text = "Copying now. This can take hours the first time.";
    else if (!hasCopyDisk) text = "No copy disk yet. Give a disk the Nightly copy job and Keel copies Files to it once a day.";
    else if (!view.can_copy) text = "Copies wait until the Files disk and a copy disk are both connected.";
    else if (!c.last_run_ms) text = "Keel makes the first copy within a minute or two.";
    else text = `Last copy ${when(c.last_run_ms)}. ${c.message}`;
    el("copystatus").textContent = text;
    el("copynow").hidden = !view.can_copy;
    el("copynow").disabled = view.copy_running;
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

  el("diskrows").addEventListener("click", (e) => {
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

  load().catch((err) => {
    el("diskstatus").textContent = String(err);
  });
  setInterval(() => {
    if (el("useform").hidden && !busy) load().catch(() => {});
  }, 10000);
})();

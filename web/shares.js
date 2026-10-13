const $ = (id) => document.getElementById(id);

function escapeHtml(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
}

async function load() {
  const [pools, shares] = await Promise.all([
    fetch("/api/pools", { credentials: "same-origin" }).then((r) => r.json()),
    fetch("/api/shares", { credentials: "same-origin" }).then((r) => r.json()),
  ]);
  $("pool").innerHTML = pools
    .map((p) => `<option value="${p.name}">${p.name} (${p.kind === "foreign" ? "another computer" : "this machine"})</option>`)
    .join("") || `<option value="">Choose disks first</option>`;
  const first = shares[0];
  if (first) {
    const addr = (first.lan && first.lan[0]) || "";
    const overlay = (first.overlay && first.overlay[0]) || "";
    const bits = [first.why];
    if (addr) bits.push(`House address: ${addr}`);
    if (overlay) bits.push(`Away address: ${overlay}`);
    if (first.username && first.password) {
      bits.push(`Name: ${first.username}  Password: ${first.password}`);
    }
    $("access").textContent = bits.filter(Boolean).join(" ");
  } else {
    // This branch is "no share published yet", not "running on Windows" — it
    // used to claim the latter, which was wrong on the appliance itself.
    $("access").textContent =
      document.documentElement.dataset.platform === "windows"
        ? "No name yet. This Windows copy cannot serve folders. The Keel computer can."
        : "No name yet. Add one, then open it from another computer.";
  }
  $("rows").innerHTML = shares
    .map((s) => {
      const addr = (s.lan && s.lan[0]) || (s.path || "");
      const ready = s.ready ? "yes" : "not yet";
      return `<tr><td><code>${escapeHtml(s.name)}</code></td><td><code>${escapeHtml(addr)}</code></td><td>${ready}</td></tr>`;
    })
    .join("") || `<tr><td colspan="3">No names yet.</td></tr>`;
}

$("form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const fd = new FormData(e.target);
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
  $("status").textContent = res.ok ? "added" : await res.text();
  await load();
});

load().catch((err) => {
  $("status").textContent = String(err);
});

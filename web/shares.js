const $ = (id) => document.getElementById(id);

async function load() {
  const [pools, shares] = await Promise.all([
    fetch("/api/pools").then((r) => r.json()),
    fetch("/api/shares").then((r) => r.json()),
  ]);
  $("pool").innerHTML = pools
    .map((p) => `<option value="${p.name}">${p.name} (${p.kind === "foreign" ? "another computer" : "this machine"})</option>`)
    .join("") || `<option value="">add a pool first</option>`;
  $("rows").innerHTML = shares
    .map(
      (s) =>
        `<tr><td><code>${s.name}</code></td><td>${s.pool}</td><td>${s.path || "whole pool"}</td></tr>`
    )
    .join("") || `<tr><td colspan="3">No shares yet.</td></tr>`;
}

$("form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const fd = new FormData(e.target);
  const res = await fetch("/api/shares", {
    method: "POST",
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

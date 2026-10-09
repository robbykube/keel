const $ = (id) => document.getElementById(id);

async function load() {
  const [power, apps] = await Promise.all([
    fetch("/api/power").then((r) => r.json()),
    fetch("/api/apps").then((r) => r.json()),
  ]);
  $("mode").textContent = `power ${power.mode}`;
  $("rows").innerHTML = apps
    .map((app) => {
      const cls = app.status === "running" ? "ssd" : app.status === "parked" ? "hdd" : "";
      return `<tr>
        <td><code>${app.name}</code></td>
        <td>${app.role}</td>
        <td>${app.runtime}</td>
        <td class="${cls}">${app.status}</td>
        <td>${app.data_paths.join(", ")}</td>
        <td>${app.idle_policy}</td>
        <td><button data-app="${app.id}" data-action="start">Start</button> <button data-app="${app.id}" data-action="park">Park</button></td>
        <td>${keelLinkTerms(app.why)}</td>
      </tr>`;
    })
    .join("");
}

$("rows").addEventListener("click", async (event) => {
  const button = event.target.closest("button[data-app]");
  if (!button) return;
  button.disabled = true;
  try {
    const res = await fetch(`/api/apps/${encodeURIComponent(button.dataset.app)}/action`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify({ action: button.dataset.action }),
    });
    const data = await res.json();
    $("mode").textContent = data.message || "action completed";
    if (data.ok) await load();
  } catch (err) {
    $("mode").textContent = String(err);
  } finally {
    button.disabled = false;
  }
});

load().catch((err) => {
  $("mode").textContent = String(err);
});

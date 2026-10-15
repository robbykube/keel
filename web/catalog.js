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

async function load() {
  const apps = await fetch("/api/apps", { credentials: "same-origin" }).then((res) => res.json());
  const choices = (Array.isArray(apps) ? apps : []).filter((app) => app.choosable);
  const added = choices.filter((app) => app.installed);
  const more = choices.filter((app) => !app.installed);
  $("added").innerHTML = added.length
    ? added.map((app) => row(app, true)).join("")
    : `<p class="meta">None yet. Add one below, or pick the first ones on Home.</p>`;
  $("more").innerHTML = more.length
    ? more.map((app) => row(app, false)).join("")
    : `<p class="meta">Every app on the list is already added.</p>`;
}

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

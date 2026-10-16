const $ = (id) => document.getElementById(id);

function show(status) {
  const names = { headscale: "A computer you run", tailscale: "Tailscale", unknown: "Not connected" };
  const states = { connected: "connected", not_configured: "not set up", unavailable: "not available here" };
  const provider = names[status.provider] || "Not connected";
  const state = states[status.state] || status.state || "";
  const address = status.overlay_ip || status.dns_name || "—";
  $("state").textContent = state;
  const control = status.control_url ? `<p class="prose">${status.control_url}</p>` : "";
  $("current").innerHTML = `<div class="k">${provider}</div><div class="v">${address}</div>${control}<p class="prose">${status.why}</p>`;
  const ready = status.state === "unavailable" || status.client_installed;
  $("install-block").hidden = ready;
  $("form").hidden = !status.client_installed;
}

async function refresh() {
  const res = await fetch("/api/overlay");
  show(await res.json());
}

let installTimer = 0;
let installWatch = false;

function showInstallProgress(progress) {
  const panel = $("install-progress");
  const message = $("install-message");
  const bar = $("install-bar");
  const fill = $("install-fill");
  if (!panel || !message || !bar || !fill) return;
  const failed = progress.state === "error";
  const visible = progress.state === "running" || progress.state === "done" || failed;
  panel.hidden = !visible;
  message.textContent = progress.message || "";
  message.classList.toggle("install-error", failed);
  const percent = Math.max(0, Math.min(100, Number(progress.percent) || 0));
  fill.style.width = `${percent}%`;
  bar.hidden = failed;
  bar.setAttribute("aria-valuenow", String(percent));
}

function stopInstallWatch() {
  installWatch = false;
  if (installTimer) {
    window.clearTimeout(installTimer);
    installTimer = 0;
  }
}

function pollInstall() {
  if (installTimer) return;
  const tick = async () => {
    installTimer = 0;
    try {
      const res = await fetch("/api/overlay/progress", { credentials: "same-origin" });
      if (res.ok) {
        const progress = await res.json();
        if (progress.state === "running") showInstallProgress(progress);
        if (progress.state === "running" || installWatch) {
          installTimer = window.setTimeout(tick, 800);
          return;
        }
      }
    } catch (_) { /* Keep the sentence on screen until Install answers. */ }
    if (installWatch) installTimer = window.setTimeout(tick, 800);
  };
  installTimer = window.setTimeout(tick, 200);
}

$("install").addEventListener("click", async () => {
  $("install").disabled = true;
  $("result").textContent = "";
  showInstallProgress({ state: "running", percent: 2, message: "Installing the Away program." });
  installWatch = true;
  pollInstall();
  try {
    const res = await fetch("/api/overlay/install", { method: "POST", credentials: "same-origin" });
    const data = await res.json().catch(() => ({ message: "request failed" }));
    stopInstallWatch();
    if (data.ok) {
      showInstallProgress({ state: "done", percent: 100, message: data.message });
      await refresh();
      $("install-progress").hidden = true;
      $("result").textContent = data.message || "";
    } else {
      showInstallProgress({ state: "error", percent: 0, message: data.message || "request failed" });
      $("install").disabled = false;
    }
  } catch (err) {
    stopInstallWatch();
    showInstallProgress({ state: "error", percent: 0, message: String(err) });
    $("install").disabled = false;
  }
});

$("form").addEventListener("submit", async (event) => {
  event.preventDefault();
  $("result").textContent = "Connecting.";
  const provider = $("provider").value;
  const res = await fetch("/api/overlay/enroll", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({
      provider,
      control_url: $("control").value || null,
      auth_key: $("key").value,
      hostname: $("hostname").value || "keel",
    }),
  });
  const data = await res.json().catch(() => ({ message: "request failed" }));
  $("result").textContent = data.message || "request failed";
  if (data.ok) {
    $("key").value = "";
    await refresh();
  }
});

refresh()
  .then(async () => {
    const res = await fetch("/api/overlay/progress", { credentials: "same-origin" });
    if (!res.ok) return;
    const progress = await res.json();
    if (progress.state !== "running") return;
    $("install-block").hidden = false;
    $("install").disabled = true;
    showInstallProgress(progress);
    installWatch = true;
    pollInstall();
  })
  .catch((err) => { $("result").textContent = String(err); });

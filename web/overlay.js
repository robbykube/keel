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

$("install").addEventListener("click", async () => {
  $("install").disabled = true;
  $("result").textContent = "Installing the Away program.";
  try {
    const res = await fetch("/api/overlay/install", { method: "POST", credentials: "same-origin" });
    const data = await res.json().catch(() => ({ message: "request failed" }));
    $("result").textContent = data.message || "request failed";
    if (data.ok) await refresh();
  } catch (err) {
    $("result").textContent = String(err);
  } finally {
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

refresh().catch((err) => { $("result").textContent = String(err); });

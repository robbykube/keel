const $ = (id) => document.getElementById(id);

function show(status) {
  const provider = status.provider || "unknown";
  const address = status.overlay_ip || status.dns_name || "—";
  $("state").textContent = `${provider} · ${status.state}`;
  const control = status.control_url ? `<p class="prose">control: ${status.control_url}</p>` : "";
  $("current").innerHTML = `<div class="k">${provider}</div><div class="v">${address}</div>${control}<p class="prose">${status.why}</p>`;
}

async function refresh() {
  const res = await fetch("/api/overlay");
  show(await res.json());
}

$("form").addEventListener("submit", async (event) => {
  event.preventDefault();
  $("result").textContent = "enrolling…";
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

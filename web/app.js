const $ = (id) => document.getElementById(id);

function bytes(n) {
  if (!n) return "—";
  const u = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
  let i = 0;
  let x = n;
  while (x >= 1024 && i < u.length - 1) {
    x /= 1024;
    i++;
  }
  return `${x.toFixed(i ? 1 : 0)} ${u[i]}`;
}

function pct(n) {
  return `${n.toFixed(0)}%`;
}

async function get(path) {
  const t0 = performance.now();
  const res = await fetch(path);
  const data = await res.json();
  const client = Math.round(performance.now() - t0);
  const server = res.headers.get("server-timing") || "";
  return { data, client, server };
}

function card(k, v) {
  return `<div class="card"><div class="k">${k}</div><div class="v">${v}</div></div>`;
}

// Where to reach this machine from another computer.
//
// Someone standing at the appliance sees a working screen and is then told to
// open it from their own PC. Without this they have no way to find out where,
// because the appliance shows no terminal and answers to no name on the
// network. This is the only place that address appears.
function showReach(s) {
  const box = $("reach");
  if (!box) return;

  const addresses = Array.isArray(s.addresses) ? s.addresses : [];
  if (!addresses.length) {
    // Say so plainly rather than hiding. A machine with no address is the
    // reason someone cannot reach it, and that is worth telling them.
    box.innerHTML = `
      <div class="reach-k">Open from another computer</div>
      <div class="reach-v">No network address yet</div>
      <div class="reach-sub">Check the network cable. This machine cannot be reached from another computer until it has an address.</div>`;
    box.hidden = false;
    return;
  }

  const port = location.port || "9478";
  const rows = addresses
    .map((ip) => `<div class="reach-v">http://${ip}:${port}/</div>`)
    .join("");
  const files = addresses.map((ip) => `<code>\\\\${ip}\\files</code>`).join(" ");

  box.innerHTML = `
    <div class="reach-k">Open from another computer</div>
    ${rows}
    <div class="reach-sub">Files from Windows File Explorer: ${files}</div>`;
  box.hidden = false;
}

async function load() {
  const [sys, disks, health, issues, power, pools, shares, overlay, updates] = await Promise.all([
    get("/api/system"),
    get("/api/disks"),
    get("/api/health"),
    get("/api/issues"),
    get("/api/power"),
    get("/api/pools"),
    get("/api/shares"),
    get("/api/overlay"),
    get("/api/updates"),
  ]);

  $("health").textContent = issues.data.red
    ? "red — Repair"
    : `${health.data.inventory} · v${health.data.version}`;
  $("timing").textContent = `system ${sys.server} / ${sys.client}ms · disks ${disks.server} / ${disks.client}ms`;

  const s = sys.data;
  showReach(s);
  $("cards").innerHTML = [
    card("Host", s.hostname),
    card("CPU", `${pct(s.cpu_pct)} · ${s.cores} cores`),
    card("Memory", `${bytes(s.mem_used)} / ${bytes(s.mem_total)}`),
    card(`Daemon <a class="term" href="/guide.html#rss">RSS</a>`, bytes(s.daemon_rss)),
  ].join("");

  const nIssues = issues.data.items.length;
  const glance = [
    `<a href="/sleep.html">${power.data.mode}</a>`,
    pools.data.length
      ? `<a href="/pools.html">${pools.data.length} pool</a>`
      : `<a href="/pools.html">no pool yet</a>`,
    shares.data.length
      ? `<a href="/shares.html">${shares.data.length} share</a>`
      : `<a href="/shares.html">no share yet</a>`,
    nIssues
      ? `<a href="/repair.html">${nIssues} on Repair</a>`
      : `<a href="/repair.html">Repair quiet</a>`,
    `<a href="/guide.html#away-from-home">away via ${overlay.data.provider}</a>`,
    `<a href="/notes.html">Notes</a>`,
  ];
  $("glance").innerHTML = glance.join(" · ");
  const update = updates.data;
  $("updates").innerHTML = update.release_url && update.update_available
    ? `<a href="${update.release_url}" target="_blank" rel="noreferrer">${update.message}</a>`
    : update.message;

  $("disks").innerHTML = disks.data
    .map((d) => {
      const media = d.rotational ? "HDD" : "SSD";
      const cls = d.rotational ? "hdd" : "ssd";
      return `<tr>
        <td><code>${d.name}</code></td>
        <td>${d.model || "—"}</td>
        <td>${bytes(d.size)}</td>
        <td>${d.transport || "—"}</td>
        <td class="${cls}">${media}</td>
        <td>${d.mount || "—"}</td>
      </tr>`;
    })
    .join("");
}

load().catch((err) => {
  $("health").textContent = String(err);
});

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

async function load() {
  const [sys, disks, health, issues, power, pools, shares] = await Promise.all([
    get("/api/system"),
    get("/api/disks"),
    get("/api/health"),
    get("/api/issues"),
    get("/api/power"),
    get("/api/pools"),
    get("/api/shares"),
  ]);

  $("health").textContent = issues.data.red
    ? "red — Repair"
    : `${health.data.inventory} · v${health.data.version}`;
  $("timing").textContent = `system ${sys.server} / ${sys.client}ms · disks ${disks.server} / ${disks.client}ms`;

  const s = sys.data;
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
    `<a href="/guide.html#away-from-home">away via Tailscale</a>`,
    `<a href="/notes.html">Notes</a>`,
  ];
  $("glance").innerHTML = glance.join(" · ");

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

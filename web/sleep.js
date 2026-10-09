const $ = (id) => document.getElementById(id);

function card(k, v) {
  return `<div class="card"><div class="k">${k}</div><div class="v">${v}</div></div>`;
}

function yn(v) {
  return v ? "yes" : "no";
}

function render(p) {
  $("cards").innerHTML = [
    card("Policy", p.policy),
    card("Mode", p.mode),
    card("Services stopped", yn(p.services_stopped)),
    card(`<a class="term" href="/guide.html#overlay">Overlay</a> required`, yn(p.overlay_required)),
  ].join("");
  $("why").innerHTML = keelLinkTerms(p.why);
}

function renderOpps(list) {
  $("opps").innerHTML = list
    .map(
      (o) => `<article class="opp">
        <h3>${o.title}</h3>
        <p>${keelLinkTerms(o.why)}</p>
        <div class="ev">${o.phase} · ${o.safe_while_working ? "safe while working" : "disruptive"} · ${keelLinkTerms(o.evidence)}</div>
      </article>`
    )
    .join("");
}

async function refresh() {
  const [p, opps] = await Promise.all([
    fetch("/api/power").then((r) => r.json()),
    fetch("/api/power/opportunities").then((r) => r.json()),
  ]);
  render(p);
  renderOpps(opps);
}

async function setMode(path, label) {
  $("status").textContent = "…";
  const res = await fetch(path, { method: "POST" });
  const p = await res.json();
  render(p);
  $("status").textContent = `mode ${p.mode} (${label})`;
  const opps = await fetch("/api/power/opportunities").then((r) => r.json());
  renderOpps(opps);
}

$("idle").addEventListener("click", () => setMode("/api/power/idle", "Tailscale floor — files still work away"));
$("active").addEventListener("click", () => setMode("/api/power/active", "demand, not all-on"));

refresh().catch((err) => {
  $("why").textContent = String(err);
});

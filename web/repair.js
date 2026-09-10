const $ = (id) => document.getElementById(id);

async function repair(id) {
  $("status").textContent = "…";
  const res = await fetch("/api/repair", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ id }),
  });
  const r = await res.json();
  $("status").innerHTML = keelLinkTerms(r.message);
  await load();
}

function issueCard(i) {
  return `<article class="opp">
    <h3><span class="${i.severity === "red" ? "hdd" : "ssd"}">${i.severity}</span> · ${i.title}</h3>
    <p>${keelLinkTerms(i.why)}</p>
    <div class="ev">${keelLinkTerms(i.evidence)}</div>
  </article>`;
}

async function load() {
  const data = await fetch("/api/issues").then((r) => r.json());
  $("banner").textContent = data.red ? "red" : "no red — yellow is expected on this PC";
  $("health").textContent = data.red ? "red" : "yellow/ok";
  $("items").innerHTML = data.items.map(issueCard).join("") || "<p class='prose'>Nothing to fix.</p>";
  $("playbook").innerHTML = data.playbook
    .map((p) => {
      const dis = p.available ? "" : " disabled";
      return `<button type="button" data-id="${p.id}"${dis}>${p.label}</button>`;
    })
    .join(" ");
  $("playbook").onclick = (e) => {
    const id = e.target.dataset && e.target.dataset.id;
    if (id) repair(id);
  };
}

load().catch((err) => {
  $("banner").textContent = String(err);
});

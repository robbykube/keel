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

// Red needs fixing; yellow is worth a look. What Keel measured stays out of
// the way, on the card's title, for whoever wants it.
function issueCard(i) {
  const red = i.severity === "red";
  const word = red ? "Needs fixing" : "Worth a look";
  const measured = String(i.evidence || "").replace(/"/g, "&quot;");
  return `<article class="opp" title="${measured}">
    <h3><span class="pill ${red ? "bad" : "warn"}">${word}</span> ${i.title}</h3>
    <p>${keelLinkTerms(i.why)}</p>
  </article>`;
}

async function load() {
  const data = await fetch("/api/issues").then((r) => r.json());
  $("banner").textContent = data.red ? "Something needs fixing" : data.items.length ? "Nothing urgent" : "All good";
  $("health").textContent = data.red ? "Needs fixing" : "No problems";
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
  showPhone();
}

async function showPhone() {
  try {
    const res = await fetch("/api/phone", { credentials: "same-origin" });
    if (!res.ok || !$("phone") || $("phone").textContent) return;
    const data = await res.json();
    if (data.on && data.server) $("phone").textContent = "Server " + data.server + ". Topic " + data.topic + ".";
  } catch (_) { /* The sentence from the button stays. */ }
}

$("phonesend").addEventListener("click", async () => {
  $("phonesend").disabled = true;
  $("phone").textContent = "Setting up the phone.";
  try {
    const res = await fetch("/api/phone", { method: "POST", credentials: "same-origin" });
    const data = await res.json().catch(() => ({ message: "The phone stayed as it was." }));
    $("phone").textContent = data.message || "The phone stayed as it was.";
  } catch (err) {
    $("phone").textContent = String(err);
  }
  $("phonesend").disabled = false;
});

load().catch((err) => {
  $("banner").textContent = String(err);
});

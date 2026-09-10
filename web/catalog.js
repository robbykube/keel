const $ = (id) => document.getElementById(id);

async function load() {
  const [power, guests] = await Promise.all([
    fetch("/api/power").then((r) => r.json()),
    fetch("/api/catalog").then((r) => r.json()),
  ]);
  $("mode").textContent = `power ${power.mode}`;
  $("rows").innerHTML = guests
    .map((g) => {
      const cls = g.status === "running" ? "ssd" : g.status === "parked" ? "hdd" : "";
      return `<tr>
        <td><code>${g.name}</code></td>
        <td>${g.role}</td>
        <td class="${cls}">${g.status}</td>
        <td>${keelLinkTerms(g.why)}</td>
      </tr>`;
    })
    .join("");
}

load().catch((err) => {
  $("mode").textContent = String(err);
});

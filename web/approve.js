const watchSignInRequests = () => {
  if (!["127.0.0.1", "localhost", "::1"].includes(location.hostname)) return;
  if (document.getElementById("sign-in-requests")) return;
  const bar = document.createElement("div");
  bar.id = "sign-in-requests";
  bar.className = "sign-in-requests";
  bar.hidden = true;
  const main = document.querySelector("main");
  const header = main && main.querySelector("header");
  if (header) header.insertAdjacentElement("afterend", bar);
  else if (main) main.prepend(bar);
  else document.body.appendChild(bar);

  const decide = async (id, allow, row) => {
    row.querySelectorAll("button").forEach((button) => { button.disabled = true; });
    try {
      const res = await fetch(`/api/auth/requests/${id}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "same-origin",
        body: JSON.stringify({ allow }),
      });
      if (!res.ok) {
        const text = await res.text();
        row.querySelector("p").textContent = text || "Could not answer that request. Press Allow again.";
        row.querySelectorAll("button").forEach((button) => { button.disabled = false; });
        return;
      }
    } catch (err) {
      row.querySelector("p").textContent = String(err.message || err);
      row.querySelectorAll("button").forEach((button) => { button.disabled = false; });
      return;
    }
    bar.dataset.ids = "";
    refresh();
  };

  const paint = (items) => {
    const ids = items.map((item) => item.id).join(" ");
    if (bar.dataset.ids === ids) return;
    bar.dataset.ids = ids;
    bar.replaceChildren();
    if (!items.length) {
      bar.hidden = true;
      return;
    }
    bar.hidden = false;
    for (const item of items) {
      const row = document.createElement("div");
      row.className = "sign-in-request";
      const text = document.createElement("p");
      text.textContent = item.message;
      const allow = document.createElement("button");
      allow.type = "button";
      allow.className = "allow";
      allow.textContent = "Allow";
      const deny = document.createElement("button");
      deny.type = "button";
      deny.textContent = "Not now";
      allow.addEventListener("click", () => decide(item.id, true, row));
      deny.addEventListener("click", () => decide(item.id, false, row));
      row.append(text, allow, deny);
      bar.append(row);
    }
  };

  async function refresh() {
    try {
      const res = await fetch("/api/auth/requests", { credentials: "same-origin" });
      if (!res.ok) {
        bar.hidden = true;
        return;
      }
      paint(await res.json());
    } catch {
      /* the screen tries again on the next tick */
    }
  }

  refresh();
  setInterval(refresh, 2000);
};

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", watchSignInRequests, { once: true });
} else {
  watchSignInRequests();
}

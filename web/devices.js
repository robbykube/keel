const status = document.querySelector("#status");
const list = document.querySelector("#list");
const button = document.querySelector("#look");

// What kind of device, in words.
const KIND = {
  router: "The router",
  this: "This computer",
  private: "A phone or tablet (it uses a made-up address on each Wi-Fi)",
  raspberry: "A Raspberry Pi",
};

const since = (ms) => {
  if (!ms) return "";
  const when = new Date(ms);
  return when.toLocaleString([], { weekday: "short", hour: "2-digit", minute: "2-digit" });
};

const read = async (res) => {
  const body = await res.text();
  try { return JSON.parse(body); } catch { return { message: body }; }
};

const mark = async (key, change) => {
  const res = await fetch("/api/devices/mark", {
    method: "POST",
    credentials: "same-origin",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ key, ...change }),
  });
  const data = await read(res);
  if (!res.ok) {
    status.textContent = data.message || "Keel could not keep that.";
    return;
  }
  paint(data);
};

const actionButton = (label, onClick) => {
  const b = document.createElement("button");
  b.type = "button";
  b.className = "texty";
  b.textContent = label;
  b.addEventListener("click", onClick);
  return b;
};

const paint = (data) => {
  status.textContent = data.message || "Press Look now.";
  list.replaceChildren();
  const devices = [...(data.devices || [])].sort((a, b) => (b.new - a.new) || (a.answering === b.answering ? 0 : a.answering ? 1 : -1));
  devices.forEach((device) => {
    const row = document.createElement("article");
    row.className = "app-row device-row";
    const body = document.createElement("div");
    const title = document.createElement("h3");
    title.textContent = device.label || (device.here ? "This computer" : device.name) || KIND[device.kind] || device.address;
    const pills = [];
    if (device.new) pills.push(["New", "warn"]);
    if (device.watched) pills.push(["Watched", ""]);
    if (!device.answering) pills.push(["Not answering", "bad"]);
    for (const [word, tone] of pills) {
      const pill = document.createElement("span");
      pill.className = `pill${tone ? ` ${tone}` : ""}`;
      pill.textContent = word;
      title.append(" ", pill);
    }
    const detail = document.createElement("p");
    const parts = [];
    if (KIND[device.kind] && (device.label || device.name)) parts.push(KIND[device.kind]);
    if (device.label && device.name && device.label !== device.name) parts.push(device.name);
    parts.push(device.address);
    if (device.mac) parts.push(device.mac);
    if (!device.answering && device.last_seen_ms) parts.push(`last answered ${since(device.last_seen_ms)}`);
    detail.textContent = parts.join(" · ");
    body.append(title, detail);
    const actions = document.createElement("div");
    actions.className = "device-actions";
    if (!device.here && device.key) {
      if (device.new) actions.appendChild(actionButton("Known", () => mark(device.key, { known: true })));
      actions.appendChild(actionButton(device.watched ? "Stop watching" : "Watch", () => mark(device.key, { watched: !device.watched })));
      actions.appendChild(actionButton(device.label ? "Rename" : "Name", () => {
        const input = document.createElement("input");
        input.type = "text";
        input.maxLength = 40;
        input.value = device.label || device.name || "";
        input.setAttribute("aria-label", "Name for this device");
        const save = actionButton("Save", () => mark(device.key, { label: input.value }));
        input.addEventListener("keydown", (e) => { if (e.key === "Enter") save.click(); });
        actions.replaceChildren(input, save);
        input.focus();
        input.select();
      }));
    }
    row.append(body, actions);
    list.appendChild(row);
  });
};

button.addEventListener("click", async () => {
  button.disabled = true;
  status.textContent = "Looking. This takes a few seconds.";
  try {
    const res = await fetch("/api/devices/look", { method: "POST", credentials: "same-origin" });
    const data = await read(res);
    if (!res.ok) {
      status.textContent = data.message || "Keel could not see the house network.";
      return;
    }
    paint(data);
  } catch {
    status.textContent = "Keel could not see the house network.";
  } finally {
    button.disabled = false;
  }
});

fetch("/api/devices", { credentials: "same-origin" })
  .then(async (res) => {
    const data = await read(res);
    if (!res.ok) {
      status.textContent = data.message || "Keel could not see the house network.";
      return;
    }
    paint(data);
  })
  .catch(() => { status.textContent = "Keel could not see the house network."; });

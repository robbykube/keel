const status = document.querySelector("#status");
const list = document.querySelector("#list");
const button = document.querySelector("#look");

const text = (value) => {
  const node = document.createElement("span");
  node.textContent = value;
  return node.innerHTML;
};

const paint = (data) => {
  status.textContent = data.message || "Press Look now.";
  list.replaceChildren();
  (data.devices || []).forEach((device) => {
    const row = document.createElement("article");
    row.className = "app-row";
    const title = device.here ? "This computer" : (device.name || device.address);
    const parts = [];
    if (device.name || device.here) parts.push(device.address);
    if (device.mac) parts.push(device.mac);
    const detail = parts.join(" · ") || "On the house network.";
    const pill = device.new ? ' <span class="pill">New</span>' : "";
    row.innerHTML = `<div><h3>${text(title)}${pill}</h3><p>${text(detail)}</p></div>`;
    list.appendChild(row);
  });
};

const read = async (res) => {
  const body = await res.text();
  try { return JSON.parse(body); } catch { return { message: body }; }
};

button.addEventListener("click", async () => {
  button.disabled = true;
  status.textContent = "Looking.";
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
  .then((res) => res.json())
  .then(paint)
  .catch(() => { status.textContent = "Keel could not see the house network."; });

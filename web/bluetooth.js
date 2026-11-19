const status = document.querySelector("#status");
const list = document.querySelector("#list");
const ask = document.querySelector("#ask");
const look = document.querySelector("#look");

const read = async (res) => {
  const body = await res.text();
  try { return JSON.parse(body); } catch { return { message: body }; }
};

const post = async (path, body) => {
  const res = await fetch(path, {
    method: "POST",
    credentials: "same-origin",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body || {}),
  });
  const data = await read(res);
  if (!res.ok) throw new Error(data.message || "Bluetooth did not answer.");
  return data;
};

const button = (label, onClick) => {
  const b = document.createElement("button");
  b.type = "button";
  b.className = "texty";
  b.textContent = label;
  b.addEventListener("click", onClick);
  return b;
};

const paint = (data) => {
  status.textContent = data.message || "Press Look.";
  ask.replaceChildren();
  ask.hidden = !data.ask;
  if (data.ask) {
    const code = document.createElement("p");
    code.className = "bluetooth-code";
    code.textContent = data.code || "";
    ask.append(code);
    if (data.ask === "yes") {
      ask.append(
        button("Yes", () => post("/api/bluetooth/answer", { answer: "yes" }).catch((e) => { status.textContent = e.message; })),
        button("No", () => post("/api/bluetooth/answer", { answer: "no" }).catch((e) => { status.textContent = e.message; })),
      );
    } else if (data.ask === "pin") {
      const input = document.createElement("input");
      input.type = "text";
      input.inputMode = "numeric";
      input.maxLength = 16;
      input.setAttribute("aria-label", "Code shown on the device");
      const save = button("Pair", () => post("/api/bluetooth/answer", { answer: input.value }).catch((e) => { status.textContent = e.message; }));
      input.addEventListener("keydown", (e) => { if (e.key === "Enter") save.click(); });
      ask.append(input, save);
    }
  }
  list.replaceChildren();
  (data.devices || []).forEach((device) => {
    const row = document.createElement("article");
    row.className = "app-row device-row";
    const body = document.createElement("div");
    const title = document.createElement("h3");
    title.textContent = device.name || "Bluetooth device";
    if (device.connected) {
      const pill = document.createElement("span");
      pill.className = "pill ok";
      pill.textContent = "Connected";
      title.append(" ", pill);
    } else if (device.paired) {
      const pill = document.createElement("span");
      pill.className = "pill";
      pill.textContent = "Paired";
      title.append(" ", pill);
    }
    const detail = document.createElement("p");
    detail.textContent = device.mac;
    body.append(title, detail);
    const actions = document.createElement("div");
    actions.className = "device-actions";
    const mac = device.mac;
    if (!device.paired) actions.append(button("Pair", () => run(() => post("/api/bluetooth/pair", { mac }))));
    else if (device.connected) actions.append(button("Disconnect", () => run(() => post("/api/bluetooth/disconnect", { mac }))));
    else actions.append(button("Connect", () => run(() => post("/api/bluetooth/connect", { mac }))));
    if (device.paired) actions.append(button("Forget", () => run(() => post("/api/bluetooth/forget", { mac }))));
    row.append(body, actions);
    list.appendChild(row);
  });
};

let timer = 0;
const schedule = (data) => {
  window.clearTimeout(timer);
  const busy = data && (data.state === "looking" || data.state === "pairing" || data.state === "confirm" || data.state === "pin" || data.state === "keyboard");
  if (busy) timer = window.setTimeout(refresh, 1000);
};

const refresh = async () => {
  const res = await fetch("/api/bluetooth", { credentials: "same-origin" });
  const data = await read(res);
  if (!res.ok) {
    status.textContent = data.message || "Bluetooth did not answer.";
    return;
  }
  paint(data);
  schedule(data);
};

const run = async (work) => {
  look.disabled = true;
  try {
    schedule({ state: "pairing" });
    const data = await work();
    paint(data);
    schedule(data);
  } catch (error) {
    status.textContent = error.message;
  } finally {
    look.disabled = false;
  }
};

look.addEventListener("click", () => run(() => post("/api/bluetooth/look")));
refresh().catch(() => { status.textContent = "Bluetooth did not answer."; });

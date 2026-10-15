const $ = (id) => document.getElementById(id);

let notes = [];
let current = null;
let timer = null;
let dirty = false;
let ticket = 0;

function label(n) {
  return n.title.trim() || n.body.trim().split("\n")[0] || "Untitled";
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
}

function renderRoster() {
  $("roster").innerHTML = notes
    .map((n) => {
      const on = n.id === current ? " active" : "";
      return `<button type="button" class="roster-item${on}" data-id="${escapeHtml(n.id)}">${escapeHtml(label(n))}</button>`;
    })
    .join("") || `<p class="prose">No notes yet.</p>`;
}

function show(id) {
  const n = notes.find((x) => x.id === id);
  if (!n) {
    current = null;
    $("title").value = "";
    $("body").value = "";
    dirty = false;
    renderRoster();
    return;
  }
  current = n.id;
  $("title").value = n.title;
  $("body").value = n.body;
  dirty = false;
  renderRoster();
}

function fields() {
  return { title: $("title").value, body: $("body").value };
}

function failText(res, text) {
  if (res.status === 401) return "Sign in first (bottom of the sidebar), then press Save.";
  const sentence = (text || "").trim();
  return sentence || "Keel could not save this note. Press Save again.";
}

async function readBody(res) {
  const text = await res.text();
  try {
    return { json: JSON.parse(text), text };
  } catch {
    return { json: null, text };
  }
}

async function createNote(payload) {
  const res = await fetch("/api/notes", {
    method: "POST",
    credentials: "same-origin",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(payload),
  });
  const data = await readBody(res);
  if (!res.ok || !data.json || !data.json.id) {
    $("status").textContent = failText(res, data.text);
    return null;
  }
  notes = notes.filter((n) => n.id !== data.json.id);
  notes.unshift(data.json);
  current = data.json.id;
  dirty = false;
  renderRoster();
  return data.json;
}

async function save() {
  clearTimeout(timer);
  const payload = fields();
  if (!current) {
    if (!payload.title.trim() && !payload.body.trim()) {
      $("status").textContent = "Write something, then press Save.";
      return false;
    }
    $("status").textContent = "Saving…";
    $("save").disabled = true;
    const created = await createNote(payload);
    $("save").disabled = false;
    if (created) $("status").textContent = "Saved.";
    return Boolean(created);
  }
  const id = current;
  const mine = ++ticket;
  $("status").textContent = "Saving…";
  $("save").disabled = true;
  let res;
  let data;
  try {
    res = await fetch(`/api/notes/${encodeURIComponent(id)}`, {
      method: "PUT",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload),
      keepalive: true,
    });
    data = await readBody(res);
  } catch (err) {
    dirty = true;
    $("save").disabled = false;
    $("status").textContent = "Keel could not save this note. Press Save again.";
    return false;
  }
  $("save").disabled = false;
  if (mine !== ticket) return true;
  if (!res.ok || !data.json) {
    dirty = true;
    $("status").textContent = failText(res, data.text);
    return false;
  }
  notes = notes.filter((n) => n.id !== data.json.id);
  notes.unshift(data.json);
  const same = current === id && $("title").value === payload.title && $("body").value === payload.body;
  if (same) {
    dirty = false;
    $("status").textContent = "Saved.";
  } else {
    dirty = true;
  }
  renderRoster();
  return true;
}

function scheduleSave() {
  dirty = true;
  $("status").textContent = "Not saved yet.";
  clearTimeout(timer);
  timer = setTimeout(() => {
    save().catch(() => {
      dirty = true;
      $("status").textContent = "Keel could not save this note. Press Save again.";
    });
  }, 600);
}

async function load() {
  const res = await fetch("/api/notes", { credentials: "same-origin" });
  if (!res.ok) {
    $("status").textContent = failText(res, await res.text());
    return;
  }
  notes = await res.json();
  if (!current && notes[0]) current = notes[0].id;
  if (current && !notes.some((n) => n.id === current)) current = notes[0] ? notes[0].id : null;
  show(current);
}

$("roster").addEventListener("click", async (event) => {
  const id = event.target.closest("button[data-id]")?.dataset.id;
  if (!id || id === current) return;
  if (dirty) {
    const ok = await save();
    if (!ok) return;
  }
  show(id);
  $("status").textContent = "";
});

$("title").addEventListener("input", scheduleSave);
$("body").addEventListener("input", scheduleSave);

$("save").addEventListener("click", () => {
  save().catch(() => {
    dirty = true;
    $("status").textContent = "Keel could not save this note. Press Save again.";
  });
});

$("new").addEventListener("click", async () => {
  clearTimeout(timer);
  const typed = fields();
  if ((!current && (typed.title.trim() || typed.body.trim())) || (current && dirty)) {
    const ok = await save();
    if (!ok) return;
  }
  const created = await createNote({ title: "", body: "" });
  if (!created) return;
  show(created.id);
  $("title").focus();
  $("status").textContent = "New note. Write it, then press Save.";
});

$("remove").addEventListener("click", async () => {
  if (!current) return;
  if (!confirm("Delete this note? It will be gone.")) return;
  const id = current;
  const res = await fetch(`/api/notes/${encodeURIComponent(id)}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
  if (!res.ok) {
    $("status").textContent = failText(res, await res.text());
    return;
  }
  notes = notes.filter((n) => n.id !== id);
  current = notes[0] ? notes[0].id : null;
  dirty = false;
  show(current);
  $("status").textContent = "Deleted.";
});

window.addEventListener("pagehide", () => {
  const payload = fields();
  const hasText = payload.title.trim() || payload.body.trim();
  if (current && dirty) {
    fetch(`/api/notes/${encodeURIComponent(current)}`, {
      method: "PUT",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload),
      keepalive: true,
    });
  } else if (!current && hasText) {
    fetch("/api/notes", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload),
      keepalive: true,
    });
  }
});

load().catch(() => {
  $("status").textContent = "Keel could not open notes. Reload the page.";
});

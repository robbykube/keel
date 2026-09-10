const $ = (id) => document.getElementById(id);

let notes = [];
let current = null;
let timer = null;
let dirty = false;

function label(n) {
  return n.title.trim() || n.body.trim().split("\n")[0] || "Untitled";
}

function renderRoster() {
  $("roster").innerHTML = notes
    .map((n) => {
      const on = n.id === current ? " active" : "";
      return `<button type="button" class="roster-item${on}" data-id="${n.id}">${escapeHtml(label(n))}</button>`;
    })
    .join("") || `<p class="prose">No notes yet.</p>`;
}

function escapeHtml(s) {
  return s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
}

function show(id) {
  const n = notes.find((x) => x.id === id);
  if (!n) {
    current = null;
    $("title").value = "";
    $("body").value = "";
    renderRoster();
    return;
  }
  current = n.id;
  $("title").value = n.title;
  $("body").value = n.body;
  dirty = false;
  renderRoster();
}

async function load() {
  notes = await fetch("/api/notes").then((r) => r.json());
  if (!current && notes[0]) current = notes[0].id;
  if (current && !notes.some((n) => n.id === current)) current = notes[0] ? notes[0].id : null;
  show(current);
}

async function save() {
  if (!current) return;
  dirty = false;
  $("status").textContent = "saving…";
  const res = await fetch(`/api/notes/${current}`, {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ title: $("title").value, body: $("body").value }),
  });
  if (!res.ok) {
    $("status").textContent = await res.text();
    return;
  }
  const n = await res.json();
  notes = notes.filter((x) => x.id !== n.id);
  notes.unshift(n);
  $("status").textContent = "saved";
  renderRoster();
}

function scheduleSave() {
  dirty = true;
  clearTimeout(timer);
  timer = setTimeout(save, 450);
}

$("roster").addEventListener("click", async (e) => {
  const id = e.target.dataset && e.target.dataset.id;
  if (!id) return;
  if (dirty) await save();
  show(id);
});

$("title").addEventListener("input", scheduleSave);
$("body").addEventListener("input", scheduleSave);

$("new").addEventListener("click", async () => {
  if (dirty) await save();
  const res = await fetch("/api/notes", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ title: "", body: "" }),
  });
  const n = await res.json();
  notes.unshift(n);
  show(n.id);
  $("title").focus();
  $("status").textContent = "new";
});

$("remove").addEventListener("click", async () => {
  if (!current) return;
  if (!confirm("Delete this note? It will be gone.")) return;
  const id = current;
  await fetch(`/api/notes/${id}`, { method: "DELETE" });
  notes = notes.filter((n) => n.id !== id);
  current = notes[0] ? notes[0].id : null;
  dirty = false;
  show(current);
  $("status").textContent = "deleted";
});

load().catch((err) => {
  $("status").textContent = String(err);
});

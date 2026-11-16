const $ = (id) => document.getElementById(id);

function escapeHtml(value) {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

async function load() {
  const note = $("note");
  const grid = $("grid");
  try {
    const res = await fetch("/api/photos/pictures", { credentials: "same-origin" });
    const data = await res.json().catch(() => ({}));
    if (!res.ok) {
      note.textContent = data.message || "Keel could not open the pictures.";
      return;
    }
    const pictures = Array.isArray(data.pictures) ? data.pictures : [];
    if (!pictures.length) {
      note.textContent = data.message || "Pictures live in Media. None are there yet.";
      return;
    }
    note.textContent = data.message || "Pictures already in Media. They stay on the disk.";
    grid.innerHTML = pictures.map((picture) => {
      const src = `/api/photos/picture?path=${encodeURIComponent(picture.path)}`;
      return `<a href="${src}"><img alt="${escapeHtml(picture.name)}" src="${src}"></a>`;
    }).join("");
  } catch (err) {
    note.textContent = String(err);
  }
}

load();

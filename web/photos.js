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
    note.textContent = data.message || "Pictures already in Media. They stay on the disk. Small copies stay on the disk with them.";
    const more = $("more");
    let shown = 0;
    const paint = () => {
      const next = pictures.slice(shown, shown + 12);
      shown += next.length;
      grid.insertAdjacentHTML("beforeend", next.map((picture) => {
        const path = encodeURIComponent(picture.path);
        const full = `/api/photos/picture?path=${path}`;
        return `<a href="${full}"><img alt="${escapeHtml(picture.name)}" src="${full}&amp;small=1" loading="lazy"></a>`;
      }).join(""));
      more.hidden = shown >= pictures.length;
    };
    more.onclick = paint;
    paint();
  } catch (err) {
    note.textContent = String(err);
  }
}

load();

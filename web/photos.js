const $ = (id) => document.getElementById(id);

function escapeHtml(value) {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function nearEnd() {
  const scroller = document.querySelector(".scroll");
  const main = document.querySelector("main");
  const el = scroller && scroller.scrollHeight > scroller.clientHeight + 8 ? scroller : main;
  if (!el) return true;
  return el.scrollTop + el.clientHeight >= el.scrollHeight - 240;
}

function showPictures(pictures) {
  const grid = $("grid");
  const more = $("more");
  const scroller = document.querySelector(".scroll");
  const main = document.querySelector("main");
  if (showPictures.onScroll) {
    scroller?.removeEventListener("scroll", showPictures.onScroll);
    main?.removeEventListener("scroll", showPictures.onScroll);
  }
  let shown = 0;
  let filling = false;
  const paint = () => {
    if (shown >= pictures.length) {
      more.hidden = true;
      return false;
    }
    const next = pictures.slice(shown, shown + 12);
    shown += next.length;
    grid.insertAdjacentHTML("beforeend", next.map((picture) => {
      const path = encodeURIComponent(picture.path);
      const full = `/api/photos/picture?path=${path}`;
      return `<a href="${full}"><img alt="${escapeHtml(picture.name)}" src="${full}&amp;small=1" loading="lazy"></a>`;
    }).join(""));
    more.hidden = shown >= pictures.length;
    return true;
  };
  const fill = () => {
    if (filling) return;
    filling = true;
    while (nearEnd() && paint()) {}
    filling = false;
  };
  showPictures.onScroll = fill;
  scroller?.addEventListener("scroll", fill, { passive: true });
  main?.addEventListener("scroll", fill, { passive: true });
  fill();
}

function stopPictures() {
  const scroller = document.querySelector(".scroll");
  const main = document.querySelector("main");
  if (showPictures.onScroll) {
    scroller?.removeEventListener("scroll", showPictures.onScroll);
    main?.removeEventListener("scroll", showPictures.onScroll);
    showPictures.onScroll = null;
  }
}

async function openCategory(path, name) {
  const note = $("note");
  const categories = $("categories");
  const grid = $("grid");
  const back = $("back");
  stopPictures();
  categories.hidden = true;
  categories.innerHTML = "";
  grid.innerHTML = "";
  back.hidden = false;
  note.textContent = name;
  try {
    const res = await fetch(`/api/photos/pictures?category=${encodeURIComponent(path)}`, { credentials: "same-origin" });
    const data = await res.json().catch(() => ({}));
    if (!res.ok) {
      note.textContent = data.message || "Keel could not open the pictures.";
      return;
    }
    const pictures = Array.isArray(data.pictures) ? data.pictures : [];
    note.textContent = pictures.length ? `${name}. ${data.message || "They stay on the disk."}` : (data.message || "No pictures in that category yet.");
    showPictures(pictures);
  } catch (err) {
    note.textContent = String(err);
  }
}

async function showCategories() {
  const note = $("note");
  const categories = $("categories");
  const grid = $("grid");
  const back = $("back");
  stopPictures();
  grid.innerHTML = "";
  back.hidden = true;
  categories.hidden = false;
  categories.innerHTML = "";
  try {
    const res = await fetch("/api/photos/pictures", { credentials: "same-origin" });
    const data = await res.json().catch(() => ({}));
    if (!res.ok) {
      note.textContent = data.message || "Keel could not open the pictures.";
      return;
    }
    const found = Array.isArray(data.categories) ? data.categories : [];
    note.textContent = data.message || "Choose a category. The pictures stay on the disk.";
    if (!found.length) return;
    for (const category of found) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "button";
      button.textContent = category.name;
      button.addEventListener("click", () => openCategory(category.path, category.name));
      categories.appendChild(button);
    }
  } catch (err) {
    note.textContent = String(err);
  }
}

$("back").addEventListener("click", showCategories);
showCategories();

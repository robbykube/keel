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

// The page the pictures scroll in: the column on a wide screen, the whole
// page on a phone.
function scrollRoot() {
  const scroller = document.querySelector(".scroll");
  if (scroller && /(auto|scroll)/.test(getComputedStyle(scroller).overflowY)) return scroller;
  return document.querySelector("main");
}

// A long category would keep every picture scrolled past in memory, and a Pi
// runs out. Pictures well away from view let go of their image and get it
// back when they come near again.
let nearby = null;

function watchNearby() {
  if (nearby || !("IntersectionObserver" in window)) return;
  nearby = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      const img = entry.target.querySelector("img");
      if (!img) continue;
      if (entry.isIntersecting) {
        if (img.dataset.src) {
          img.src = img.dataset.src;
          delete img.dataset.src;
        }
      } else if (img.getAttribute("src")) {
        img.dataset.src = img.getAttribute("src");
        img.removeAttribute("src");
      }
    }
  }, { root: scrollRoot(), rootMargin: "1200px 0px" });
}

function stopNearby() {
  if (!nearby) return;
  nearby.disconnect();
  nearby = null;
}

// "November 2026", for the heading over a month of pictures. The camera's
// clock has no time zone, so the date is read as it was written.
function monthOf(taken) {
  if (!taken) return "No date";
  return new Date(taken * 1000).toLocaleDateString(undefined, { month: "long", year: "numeric", timeZone: "UTC" });
}

function pictureUrl(picture, extra = "") {
  const v = picture.v ? `&v=${encodeURIComponent(picture.v)}` : "";
  return `/api/photos/picture?path=${encodeURIComponent(picture.path)}${extra}${v}`;
}

function tileFor(picture, index) {
  const name = escapeHtml(picture.name);
  const full = escapeHtml(pictureUrl(picture));
  const small = escapeHtml(pictureUrl(picture, "&small=1"));
  if (picture.video) {
    // A frame from the video, once Keel has the video tool; until then the name.
    return `<a href="${full}" class="photo-video" aria-label="Video: ${name}" data-index="${index}">`
      + `<img alt="" src="${small}" loading="lazy">`
      + `<span class="video-mark" aria-hidden="true"></span><span class="video-name">${name}</span></a>`;
  }
  const screen = escapeHtml(pictureUrl(picture, "&size=screen"));
  return `<a href="${full}" aria-label="${name}" data-index="${index}">`
    + `<img alt="" src="${small}" loading="lazy"></a>`;
}

// The pictures of the open category, for Previous and Next.
let album = [];
let current = -1;

function showPictures(pictures) {
  album = pictures;
  const grid = $("grid");
  const more = $("more");
  const scroller = document.querySelector(".scroll");
  const main = document.querySelector("main");
  if (showPictures.onScroll) {
    scroller?.removeEventListener("scroll", showPictures.onScroll);
    main?.removeEventListener("scroll", showPictures.onScroll);
  }
  stopNearby();
  watchNearby();
  let shown = 0;
  let filling = false;
  let month = null;
  const paint = () => {
    if (shown >= pictures.length) {
      more.hidden = true;
      return false;
    }
    const next = pictures.slice(shown, shown + 12);
    const before = grid.children.length;
    shown += next.length;
    const first = shown - next.length;
    grid.insertAdjacentHTML("beforeend", next.map((picture, offset) => {
      const label = monthOf(picture.taken);
      const heading = label !== month ? `<h2 class="photo-month">${escapeHtml(label)}</h2>` : "";
      month = label;
      return heading + tileFor(picture, first + offset);
    }).join(""));
    if (nearby) {
      for (const link of Array.from(grid.children).slice(before)) {
        if (link.tagName === "A") nearby.observe(link);
      }
    }
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
  stopNearby();
  const scroller = document.querySelector(".scroll");
  const main = document.querySelector("main");
  if (showPictures.onScroll) {
    scroller?.removeEventListener("scroll", showPictures.onScroll);
    main?.removeEventListener("scroll", showPictures.onScroll);
    showPictures.onScroll = null;
  }
}

// Grey shapes where the pictures will be, so a slow first visit on a small
// computer still shows that something is coming.
function placeholders(count, className) {
  return Array.from({ length: count }, () => `<span class="${className} is-waiting" aria-hidden="true"></span>`).join("");
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
  grid.innerHTML = placeholders(12, "photo-waiting");
  try {
    const res = await fetch(`/api/photos/pictures?category=${encodeURIComponent(path)}`, { credentials: "same-origin" });
    const data = await res.json().catch(() => ({}));
    if (!res.ok) {
      grid.innerHTML = "";
      note.textContent = data.message || "Keel could not open the pictures.";
      return;
    }
    const pictures = Array.isArray(data.pictures) ? data.pictures : [];
    grid.innerHTML = "";
    note.textContent = pictures.length ? `${name}. ${data.message || "They stay on the disk."}` : (data.message || "No pictures in that category yet.");
    showPictures(pictures);
  } catch (err) {
    grid.innerHTML = "";
    note.textContent = String(err);
  }
}

async function showCategories() {
  closePicture();
  const note = $("note");
  const categories = $("categories");
  const grid = $("grid");
  const back = $("back");
  stopPictures();
  grid.innerHTML = "";
  back.hidden = true;
  categories.hidden = false;
  categories.innerHTML = placeholders(4, "category-waiting");
  try {
    const res = await fetch("/api/photos/pictures", { credentials: "same-origin" });
    const data = await res.json().catch(() => ({}));
    categories.innerHTML = "";
    if (!res.ok) {
      note.textContent = data.message || "Keel could not open the pictures.";
      return;
    }
    const found = Array.isArray(data.categories) ? data.categories : [];
    note.textContent = data.message || "Choose a category. The pictures stay on the disk.";
    if (!found.length) return;
    for (const category of found) {
      const tile = document.createElement("button");
      tile.type = "button";
      tile.className = "category-tile";
      tile.title = category.name;
      const frame = document.createElement("span");
      frame.className = "category-cover";
      if (category.cover) {
        const cover = document.createElement("img");
        cover.alt = "";
        cover.loading = "lazy";
        cover.src = pictureUrl({ path: category.cover, v: category.cover_v }, "&small=1");
        cover.addEventListener("error", () => cover.remove());
        frame.appendChild(cover);
      }
      const name = document.createElement("span");
      name.className = "category-name";
      name.textContent = category.name;
      tile.append(frame, name);
      tile.addEventListener("click", () => openCategory(category.path, category.name));
      categories.appendChild(tile);
    }
  } catch (err) {
    categories.innerHTML = "";
    note.textContent = String(err);
  }
}

let viewToken = 0;

// Try a crop. Nothing is saved: the frame only shows what a shape would keep.
// The frame is kept as its middle (cx, cy, as parts of the picture) and its
// size (a part of the largest frame of that shape that fits).
const crop = { ratio: 0, round: false, cx: 0.5, cy: 0.5, scale: 1, drag: null };
const SMALLEST = 0.15;

function imageBox() {
  const stage = $("viewer-stage");
  const image = $("viewer-image");
  const w = stage.clientWidth;
  const h = stage.clientHeight;
  const nw = image.naturalWidth;
  const nh = image.naturalHeight;
  if (!w || !h || !nw || !nh) return null;
  const scale = Math.min(w / nw, h / nh);
  const iw = nw * scale;
  const ih = nh * scale;
  return { left: (w - iw) / 2, top: (h - ih) / 2, width: iw, height: ih };
}

function frameBox(box) {
  let fw = box.width;
  let fh = fw / crop.ratio;
  if (fh > box.height) {
    fh = box.height;
    fw = fh * crop.ratio;
  }
  fw *= crop.scale;
  fh *= crop.scale;
  // Keep the whole frame on the picture.
  const half = { x: fw / 2 / box.width, y: fh / 2 / box.height };
  crop.cx = Math.min(1 - half.x, Math.max(half.x, crop.cx));
  crop.cy = Math.min(1 - half.y, Math.max(half.y, crop.cy));
  return { width: fw, height: fh, left: crop.cx * box.width - fw / 2, top: crop.cy * box.height - fh / 2 };
}

function drawCrop() {
  const area = $("crop-area");
  const frame = $("crop-frame");
  $("crop-note").hidden = !crop.ratio;
  const box = crop.ratio ? imageBox() : null;
  if (!box) {
    area.hidden = true;
    return;
  }
  area.hidden = false;
  Object.assign(area.style, { left: `${box.left}px`, top: `${box.top}px`, width: `${box.width}px`, height: `${box.height}px` });
  const f = frameBox(box);
  Object.assign(frame.style, { left: `${f.left}px`, top: `${f.top}px`, width: `${f.width}px`, height: `${f.height}px` });
  frame.classList.toggle("is-round", crop.round);
  $("crop-size").value = String(Math.round(crop.scale * 100));
}

function chooseShape(button) {
  for (const other of $("crop-shapes").querySelectorAll("button")) {
    other.setAttribute("aria-pressed", String(other === button));
  }
  crop.ratio = Number(button.dataset.ratio) || 0;
  crop.round = button.dataset.round === "1";
  crop.cx = 0.5;
  crop.cy = 0.5;
  crop.scale = 1;
  drawCrop();
}

function moveCrop(dx, dy) {
  const box = imageBox();
  if (!box || !crop.ratio) return;
  crop.cx += dx / box.width;
  crop.cy += dy / box.height;
  drawCrop();
}

// A new size, keeping the frame's middle where it is.
function sizeCrop(scale) {
  crop.scale = Math.min(1, Math.max(SMALLEST, scale));
  drawCrop();
}

// Dragging the corner: the opposite corner stays where it is.
function stretchCrop(dx) {
  const box = imageBox();
  if (!box || !crop.ratio) return;
  const before = frameBox(box);
  const full = before.width / crop.scale;
  const scale = Math.min(1, Math.max(SMALLEST, (before.width + dx) / full));
  const width = full * scale;
  const height = (before.height / before.width) * width;
  crop.scale = scale;
  crop.cx = (before.left + width / 2) / box.width;
  crop.cy = (before.top + height / 2) / box.height;
  drawCrop();
}

// Shows one picture of the album, and gets the next one ready while it is
// looked at, so stepping through does not wait on the copy.
let ahead = null;

// A browser opens only a few connections to Keel at once. While a picture is
// open, thumbnails still loading behind it let go, so the open picture is not
// stuck waiting; they come back when the viewer closes.
function pauseGrid() {
  for (const img of $("grid").querySelectorAll("img[src]")) {
    if (img.complete) continue;
    img.dataset.src = img.getAttribute("src");
    img.removeAttribute("src");
  }
}

function resumeGrid() {
  if (!nearby) {
    for (const img of $("grid").querySelectorAll("img[data-src]")) {
      img.src = img.dataset.src;
      delete img.dataset.src;
    }
    return;
  }
  // Watching again reports what is near view, and those load again.
  for (const link of $("grid").querySelectorAll("a[data-index]")) {
    if (!link.querySelector("img[data-src]")) continue;
    nearby.unobserve(link);
    nearby.observe(link);
  }
}

function showAt(index) {
  const picture = album[index];
  if (!picture) return;
  if ($("viewer").hidden) pauseGrid();
  current = index;
  ahead = null;
  if (picture.video) openVideo(pictureUrl(picture), picture.name, pictureUrl(picture, "&size=screen"));
  else openPicture(pictureUrl(picture, "&small=1"), pictureUrl(picture, "&size=screen"), picture.name);
  $("viewer-prev").disabled = index <= 0;
  $("viewer-next").disabled = index >= album.length - 1;
  $("viewer-count").textContent = `${index + 1} of ${album.length}`;
}

function step(by) {
  const index = current + by;
  if (index < 0 || index >= album.length || $("viewer").hidden) return;
  showAt(index);
}

function getReady(index) {
  const picture = album[index];
  if (!picture || picture.video) return;
  ahead = new Image();
  ahead.src = pictureUrl(picture, "&size=screen");
}

function openVideo(url, name, poster) {
  viewToken += 1;
  stopLoading();
  const video = $("viewer-video");
  $("viewer-image").hidden = true;
  $("viewer-image").removeAttribute("src");
  $("crop-shapes").hidden = true;
  $("crop-area").hidden = true;
  $("crop-note").hidden = true;
  video.hidden = false;
  video.setAttribute("aria-label", name);
  if (poster) video.poster = poster;
  else video.removeAttribute("poster");
  video.src = url;
  $("viewer").hidden = false;
  $("viewer-back").focus();
}

function stopVideo() {
  const video = $("viewer-video");
  video.pause();
  video.removeAttribute("src");
  video.removeAttribute("poster");
  video.load();
  video.hidden = true;
}

function openPicture(small, screen, name) {
  const token = ++viewToken;
  const viewer = $("viewer");
  const image = $("viewer-image");
  stopVideo();
  image.hidden = false;
  $("crop-shapes").hidden = false;
  image.alt = name;
  crop.cx = 0.5;
  crop.cy = 0.5;
  $("crop-area").hidden = true;
  image.src = small;
  viewer.hidden = false;
  $("viewer-back").focus();
  // The screen copy, not the camera original: a Pi cannot hold many of those.
  stopLoading();
  const next = new Image();
  next.onload = () => {
    if (token !== viewToken || viewer.hidden) return;
    image.src = screen;
    loading = null;
    getReady(current + 1);
  };
  next.src = screen;
  loading = next;
}

let loading = null;

function stopLoading() {
  if (!loading) return;
  loading.onload = null;
  loading.src = "";
  loading = null;
}

function closePicture() {
  viewToken += 1;
  stopLoading();
  ahead = null;
  // Leave the grid at the picture that was open last.
  if (current >= 0 && !$("viewer").hidden) {
    $("grid").querySelector(`a[data-index="${current}"]`)?.scrollIntoView({ block: "nearest" });
    setTimeout(resumeGrid, 0);
  }
  const viewer = $("viewer");
  const image = $("viewer-image");
  viewer.hidden = true;
  image.removeAttribute("src");
  stopVideo();
}

$("grid").addEventListener("click", (event) => {
  const link = event.target.closest("a");
  if (!link || !$("grid").contains(link)) return;
  event.preventDefault();
  showAt(Number(link.dataset.index));
});
// A copy Keel could not make (no video tool yet, a damaged file) leaves a
// plain tile instead of a broken picture.
$("grid").addEventListener("error", (event) => {
  const img = event.target;
  if (img.tagName !== "IMG" || !img.getAttribute("src")) return;
  if (img.closest(".photo-video")) img.remove();
  else img.classList.add("is-missing");
}, true);
$("back").addEventListener("click", showCategories);
$("viewer-back").addEventListener("click", closePicture);
$("viewer-image").addEventListener("load", drawCrop);
window.addEventListener("resize", drawCrop);
$("crop-shapes").addEventListener("click", (event) => {
  const button = event.target.closest("button");
  if (button) chooseShape(button);
});
$("crop-frame").addEventListener("pointerdown", (event) => {
  const stretch = event.target === $("crop-handle");
  crop.drag = { id: event.pointerId, x: event.clientX, y: event.clientY, stretch };
  event.currentTarget.setPointerCapture(event.pointerId);
  event.preventDefault();
});
$("crop-frame").addEventListener("pointermove", (event) => {
  if (!crop.drag || crop.drag.id !== event.pointerId) return;
  const dx = event.clientX - crop.drag.x;
  const dy = event.clientY - crop.drag.y;
  crop.drag.x = event.clientX;
  crop.drag.y = event.clientY;
  if (crop.drag.stretch) stretchCrop(Math.abs(dx) >= Math.abs(dy) ? dx : dy * crop.ratio);
  else moveCrop(dx, dy);
});
$("crop-frame").addEventListener("wheel", (event) => {
  event.preventDefault();
  sizeCrop(crop.scale * (event.deltaY < 0 ? 1.06 : 1 / 1.06));
}, { passive: false });
$("crop-size").addEventListener("input", (event) => sizeCrop(Number(event.target.value) / 100));
for (const end of ["pointerup", "pointercancel"]) {
  $("crop-frame").addEventListener(end, () => { crop.drag = null; });
}
$("crop-frame").addEventListener("keydown", (event) => {
  const step = event.shiftKey ? 40 : 10;
  const moves = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
  if (event.key === "+" || event.key === "=") {
    event.preventDefault();
    sizeCrop(crop.scale * 1.06);
    return;
  }
  if (event.key === "-") {
    event.preventDefault();
    sizeCrop(crop.scale / 1.06);
    return;
  }
  const move = moves[event.key];
  if (!move) return;
  event.preventDefault();
  moveCrop(...move);
});
$("viewer-prev").addEventListener("click", () => step(-1));
$("viewer-next").addEventListener("click", () => step(1));
document.addEventListener("keydown", (event) => {
  if ($("viewer").hidden) return;
  if (event.key === "Escape") closePicture();
  // The crop frame and a playing video use the arrow keys themselves.
  if (event.target === $("crop-frame") || event.target === $("viewer-video")) return;
  if (event.key === "ArrowLeft") step(-1);
  if (event.key === "ArrowRight") step(1);
});
// A swipe across the picture steps through, when no crop shape is chosen.
let swipe = null;
$("viewer-stage").addEventListener("pointerdown", (event) => {
  if (crop.ratio || !$("viewer-video").hidden || event.target === $("crop-frame")) return;
  swipe = { x: event.clientX, y: event.clientY };
});
$("viewer-stage").addEventListener("pointerup", (event) => {
  if (!swipe) return;
  const dx = event.clientX - swipe.x;
  const dy = event.clientY - swipe.y;
  swipe = null;
  if (Math.abs(dx) > 60 && Math.abs(dx) > Math.abs(dy) * 1.5) step(dx < 0 ? 1 : -1);
});
$("viewer-stage").addEventListener("pointercancel", () => { swipe = null; });
showCategories();

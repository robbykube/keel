const frame = document.getElementById("frame");
const back = document.getElementById("back");
const full = document.getElementById("full");

const page = () => {
  const raw = new URLSearchParams(location.search).get("to") || "";
  try {
    const url = new URL(raw);
    if (url.protocol === "http:" || url.protocol === "https:") return url.href;
  } catch { /* not a page address */ }
  return "";
};

const paintFull = () => {
  full.textContent = document.fullscreenElement ? "Leave full screen" : "Full screen";
};

back.addEventListener("click", async () => {
  if (document.fullscreenElement) {
    try { await document.exitFullscreen(); } catch { /* already out */ }
  }
  let dest = "/";
  try {
    const saved = sessionStorage.getItem("keel-back") || "";
    if (saved.startsWith("/") && !saved.startsWith("//") && !saved.startsWith("/open.html")) dest = saved;
  } catch { /* the home page */ }
  location.href = dest;
});

full.addEventListener("click", async () => {
  if (document.fullscreenElement) {
    try { await document.exitFullscreen(); } catch { /* stay as it is */ }
    return;
  }
  try { await document.documentElement.requestFullscreen(); } catch { /* the bar stays either way */ }
});

document.addEventListener("fullscreenchange", paintFull);

const address = page();
if (address) frame.src = address;
else frame.replaceWith(Object.assign(document.createElement("p"), { className: "open-empty", textContent: "Nothing to open." }));
paintFull();

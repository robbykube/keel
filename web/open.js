// The Keel window. Two ways in:
//   /open.html?app=photos  Open on an app: start it if needed, then show it.
//   /open.html?to=<url>    A link that leaves Keel, from the Keel screen.
// Either way the bar stays: Back, the app's name, New tab, Full screen.

const $ = (id) => document.getElementById(id);
const frame = $("frame");
const full = $("full");

// Same ids as the Apps page. `path` is the app's status; `/install` starts it.
const APPS = {
  photos: { title: "Photos", path: "/api/apps/immich" },
  watch: { title: "Watch", path: "/api/apps/jellyfin" },
  papers: { title: "Papers", path: "/api/apps/papra" },
  mail: { title: "Mail", path: "/api/apps/thunderbird" },
  passwords: { title: "Passwords", path: "/api/apps/vaultwarden" },
  video: { title: "Video", path: "/api/apps/jitsi" },
};

const params = new URLSearchParams(location.search);

const goBack = () => {
  let dest = params.get("app") ? "/catalog.html" : "/";
  try {
    const saved = sessionStorage.getItem("keel-back") || "";
    if (saved.startsWith("/") && !saved.startsWith("//") && !saved.startsWith("/open.html")) dest = saved;
  } catch { /* the default page */ }
  location.href = dest;
};

$("back").addEventListener("click", async () => {
  if (document.fullscreenElement) {
    try { await document.exitFullscreen(); } catch { /* already out */ }
  }
  goBack();
});

const paintFull = () => {
  full.textContent = document.fullscreenElement ? "Leave full screen" : "Full screen";
};
full.addEventListener("click", async () => {
  if (document.fullscreenElement) {
    try { await document.exitFullscreen(); } catch { /* stay as it is */ }
    return;
  }
  try { await document.documentElement.requestFullscreen(); } catch { /* the bar stays either way */ }
});
document.addEventListener("fullscreenchange", paintFull);
paintFull();

const show = (url, plain) => {
  $("wait").hidden = true;
  frame.hidden = false;
  frame.src = url;
  if (plain) {
    $("newtab").href = plain;
    $("newtab").hidden = false;
  }
};

// One sentence and a bar while the app starts (ui.md).
let shown = 0;
const waitFor = (title, message, percent) => {
  $("ask").hidden = true;
  $("wait").classList.remove("asking");
  $("wait").hidden = false;
  frame.hidden = true;
  $("wait-title").textContent = title;
  $("wait-message").textContent = message;
  $("wait-actions").hidden = true;
  $("wait-bar").hidden = false;
  shown = Math.max(shown, Math.min(99, percent || 0));
  $("wait-fill").style.width = `${shown}%`;
  $("wait-bar").setAttribute("aria-valuenow", String(Math.round(shown)));
};

const failed = (title, said) => {
  $("ask").hidden = true;
  $("wait").classList.remove("asking");
  $("wait").hidden = false;
  frame.hidden = true;
  $("wait-title").textContent = title;
  const help = said && typeof keelHelp === "function" ? ` ${keelHelp(said, "What the computer said")}` : "";
  $("wait-message").innerHTML = `${title} did not start. Try again, or open Logs to see why.${help}`;
  $("wait-bar").hidden = true;
  $("wait-actions").hidden = false;
};

const json = async (path, init) => {
  const res = await fetch(path, { credentials: "same-origin", ...(init || {}) });
  const data = await res.json().catch(() => ({}));
  return { res, data };
};

const pause = (ms) => new Promise((done) => setTimeout(done, ms));

// Keel has no sign-in for this app yet: ask once, and Keel keeps it.
const askOnce = (id, spec, launch) => {
  $("wait").hidden = false;
  $("wait").classList.add("asking");
  frame.hidden = true;
  $("wait-title").textContent = spec.title;
  $("wait-message").textContent = launch.message || `Sign in to ${spec.title} once.`;
  $("wait-bar").hidden = true;
  $("wait-actions").hidden = true;
  $("ask").hidden = false;
  $("ask-word").textContent = launch.ask || "Name";
  $("ask-user").type = launch.ask === "Email" ? "email" : "text";
  $("ask-status").textContent = "";
  $("ask-skip").href = launch.plain || "#";
  $("ask-skip").onclick = (event) => {
    event.preventDefault();
    show(launch.plain, launch.plain);
  };
  $("ask").onsubmit = async (event) => {
    event.preventDefault();
    $("ask-go").disabled = true;
    $("ask-status").textContent = "Checking with " + spec.title + ".";
    const { data } = await json(`/api/apps/${id}/remember`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ user: $("ask-user").value, password: $("ask-pass").value }),
    });
    $("ask-go").disabled = false;
    if (!data.ok) {
      $("ask-status").textContent = data.message || `${spec.title} did not accept that.`;
      return;
    }
    $("ask-pass").value = "";
    openApp(id);
  };
  // On the Keel screen, focus would raise the keys over the sentence before
  // it is read; there the person taps the box.
  if (!document.body.classList.contains("keel-screen")) $("ask-user").focus();
};

// On the Keel screen (where the keyboard appears) the form keeps room below
// it all the time, so the keys opening or closing never move the buttons.
if (["127.0.0.1", "localhost", "::1"].includes(location.hostname.replace(/^\[|\]$/g, ""))) {
  document.body.classList.add("keel-screen");
}

// On the Keel screen the keyboard rises over the window: leave it room and
// keep the box being typed in above it.
document.addEventListener("focusin", (event) => {
  if (!event.target.closest || !event.target.closest("#ask")) return;
  window.setTimeout(() => {
    const board = document.querySelector(".screen-keyboard");
    if (board && !board.hidden) document.body.style.setProperty("--keyboard-room", `${board.offsetHeight + 16}px`);
    event.target.closest("label").scrollIntoView({ block: "start" });
  }, 60);
});

let restarted = false;

async function openApp(id) {
  const spec = APPS[id];
  if (!spec) {
    failed("That app", "Keel does not know that app.");
    return;
  }
  document.title = `${spec.title} · Keel`;
  $("title").textContent = spec.title;
  shown = 0;
  waitFor(spec.title, `Opening ${spec.title}.`, 4);

  let { data: status } = await json(spec.path);
  if (status.local) {
    location.replace("/photos.html");
    return;
  }
  if (!status.ready) {
    if (status.state !== "running") {
      const { res, data } = await json(`${spec.path}/install`, { method: "POST" });
      if (res.status === 401) {
        location.href = `/login.html?next=${encodeURIComponent(location.pathname + location.search)}`;
        return;
      }
      if (!res.ok || data.ok === false) {
        failed(spec.title, data.message || "");
        return;
      }
    }
    // Wait for it, up to ten minutes (the first time downloads the program).
    const until = Date.now() + 10 * 60 * 1000;
    while (Date.now() < until) {
      ({ data: status } = await json(spec.path));
      if (status.ready) break;
      if (status.state === "error") {
        failed(spec.title, status.message || "");
        return;
      }
      if (status.state === "unavailable" || status.state === "refused" || status.allowed === false) {
        failed(spec.title, status.message || "");
        return;
      }
      waitFor(spec.title, status.message || `Opening ${spec.title}.`, status.percent || shown + 1);
      await pause(900);
    }
    if (!status.ready) {
      failed(spec.title, "It took longer than ten minutes.");
      return;
    }
  }

  waitFor(spec.title, `Opening ${spec.title}.`, 99);
  const { res, data: launch } = await json(`/api/apps/${id}/launch`, { method: "POST" });
  if (res.status === 401) {
    location.href = `/login.html?next=${encodeURIComponent(location.pathname + location.search)}`;
    return;
  }
  if (launch.restart && !restarted) {
    restarted = true;
    waitFor(spec.title, launch.message || `${spec.title} is starting again.`, 10);
    await json(`/api/apps/${id}/stop`, { method: "POST" });
    shown = 0;
    openApp(id);
    return;
  }
  if (launch.ask) {
    askOnce(id, spec, launch);
    return;
  }
  if (!res.ok || !launch.ok) {
    failed(spec.title, launch.message || "");
    return;
  }
  // A camera, a microphone, or the password vault need a locked page. A
  // window inside a plain page cannot give them one, and a locked page
  // cannot hold a plain one, so those open as the whole page instead.
  const plainInLocked = location.protocol === "https:" && launch.url.startsWith("http:");
  if ((launch.secure && !window.isSecureContext) || plainInLocked) {
    location.href = launch.url;
    return;
  }
  show(launch.url, launch.plain);
}

$("again").addEventListener("click", () => openApp(params.get("app") || ""));

const outside = () => {
  const raw = params.get("to") || "";
  try {
    const url = new URL(raw);
    if (url.protocol === "http:" || url.protocol === "https:") return url.href;
  } catch { /* not a page address */ }
  return "";
};

if (params.get("app")) {
  openApp(params.get("app"));
} else if (outside()) {
  show(outside(), "");
} else {
  frame.replaceWith(Object.assign(document.createElement("p"), { className: "open-empty", textContent: "Nothing to open." }));
}

const PRIMARY = [
  ["/", "Home"],
  ["/shares.html", "Files"],
  ["/catalog.html", "Apps"],
  ["/overlay.html", "Away"],
];
const computerNav = (showInstall) => {
  const items = [["/pools.html", "Disks"], ["/devices.html", "Devices"]];
  if (showInstall) items.push(["/installer.html", "Install"]);
  items.push(["/repair.html", "Repair"]);
  return items;
};
const SUPPORT = [
  ["/guide.html", "Guide"],
  ["/logs.html", "Logs"],
  ["/settings.html", "Settings"],
  ["/login.html", "Sign in"],
];
const NOTES = [["/notes.html", "Notes"]];

const NAV_ICONS = {
  Home: '<svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="m4 10 8-6 8 6v9a1 1 0 0 1-1 1h-4v-6H9v6H5a1 1 0 0 1-1-1z"/></svg>',
  Files: '<svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M3.5 7.5h6l2 2h9v8.8a1.2 1.2 0 0 1-1.2 1.2H4.7a1.2 1.2 0 0 1-1.2-1.2z"/></svg>',
  Apps: '<svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="4" width="6" height="6" rx="1"/><rect x="14" y="4" width="6" height="6" rx="1"/><rect x="4" y="14" width="6" height="6" rx="1"/><rect x="14" y="14" width="6" height="6" rx="1"/></svg>',
  Away: '<svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="8.5"/><path d="M3.5 12h17M12 3.5a13 13 0 0 1 0 17M12 3.5a13 13 0 0 0 0 17"/></svg>',
  Guide: '<svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 4.5h10.5A3.5 3.5 0 0 1 19 8v11.5H8.5A3.5 3.5 0 0 0 5 23z"/><path d="M5 4.5v15A3.5 3.5 0 0 1 8.5 16H19M8 8h7M8 11.5h7"/></svg>',
  Notes: '<svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M6 3.5h12a1.5 1.5 0 0 1 1.5 1.5v14a1.5 1.5 0 0 1-1.5 1.5H6A1.5 1.5 0 0 1 4.5 19V5A1.5 1.5 0 0 1 6 3.5z"/><path d="M8 8h8M8 12h8M8 16h5"/></svg>',
  Devices: '<svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><circle cx="6" cy="12" r="2"/><circle cx="18" cy="7" r="2"/><circle cx="18" cy="17" r="2"/><path d="M8 12h8M16.2 8.2 8 12l8.2 3.8"/></svg>',
  Disks: '<svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><ellipse cx="12" cy="6.5" rx="7" ry="2.5"/><path d="M5 6.5v11c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5v-11M5 12c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5"/></svg>',
  Install: '<svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 4v11M7.5 10.5 12 15l4.5-4.5M4.5 16.5v2a1.5 1.5 0 0 0 1.5 1.5h12a1.5 1.5 0 0 0 1.5-1.5v-2"/></svg>',
  Repair: '<svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M14.5 4.2a4.8 4.8 0 0 0-3.9 6.4l-6 6a1.9 1.9 0 0 0 2.7 2.7l6-6a4.8 4.8 0 0 0 6.4-3.9l-2.9 1.4-2.3-1.1-1.1-2.3z"/></svg>',
  Logs: '<svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M9 6.5h10.5M9 12h10.5M9 17.5h7"/><circle cx="5.2" cy="6.5" r="1.1"/><circle cx="5.2" cy="12" r="1.1"/><circle cx="5.2" cy="17.5" r="1.1"/></svg>',
  Settings: '<svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M4.5 7h8.5M17.5 7h2M4.5 12h3M11.5 12h8M4.5 17h7M15.5 17h4"/><circle cx="15.2" cy="7" r="2.2"/><circle cx="9.5" cy="12" r="2.2"/><circle cx="13.5" cy="17" r="2.2"/></svg>',
  "Sign in": '<svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M13.5 4.5h4a1.5 1.5 0 0 1 1.5 1.5v12a1.5 1.5 0 0 1-1.5 1.5h-4M4.5 12h10M11 8.5l3.5 3.5-3.5 3.5"/></svg>',
};

const BRAND_ICON = '<svg class="brand-mark" viewBox="0 0 24 24" aria-hidden="true"><rect x="3.5" y="5" width="17" height="13" rx="2.5"/><path d="M7 9.5h10M7 13h5"/></svg>';

const initialTheme = (() => {
  try { return localStorage.getItem("keel-theme") || "dark"; } catch { return "dark"; }
})();
document.documentElement.dataset.theme = initialTheme === "light" ? "light" : "dark";

const currentPath = () => location.pathname || "/";

const isActive = (href) => {
  const current = currentPath();
  if (href === "/") return current === "/" || current === "/index.html";
  return current === href;
};

const navLink = (href, label, extraClass = "") => {
  const link = document.createElement("a");
  link.href = href;
  link.innerHTML = `${NAV_ICONS[label] || ""}<span class="nav-label">${label}</span>`;
  link.className = [extraClass, isActive(href) ? "active" : ""].filter(Boolean).join(" ");
  if (isActive(href)) link.setAttribute("aria-current", "page");
  return link;
};

const themeControl = () => {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "theme-toggle";
  button.setAttribute("aria-label", "Switch colour theme");
  const paint = () => {
    const light = document.documentElement.dataset.theme === "light";
    button.innerHTML = `${light ? '<svg class="theme-icon" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>' : '<svg class="theme-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M19.5 15.5A8 8 0 0 1 8.5 4.5 8.5 8.5 0 1 0 19.5 15.5z"/></svg>'}<span>${light ? "Light" : "Dark"}</span>`;
    button.setAttribute("aria-pressed", String(light));
  };
  button.addEventListener("click", () => {
    const next = document.documentElement.dataset.theme === "light" ? "dark" : "light";
    document.documentElement.dataset.theme = next;
    try { localStorage.setItem("keel-theme", next); } catch { /* private browsing */ }
    paint();
  });
  paint();
  return button;
};

const computerGroup = (items) => {
  const group = document.createElement("div");
  group.className = "nav-group";
  const title = document.createElement("p");
  title.className = "nav-group-title";
  title.textContent = "This computer";
  group.appendChild(title);
  items.forEach(([href, label]) => group.appendChild(navLink(href, label)));
  return group;
};

const sectionLinks = (className, ariaLabel, items) => {
  const section = document.createElement("section");
  section.className = className;
  section.setAttribute("aria-label", ariaLabel);
  items.forEach(([href, label]) => section.appendChild(navLink(href, label, "quiet")));
  return section;
};

const POWER_ICON =
  '<svg width="16" height="16" viewBox="0 0 24 24" aria-hidden="true"><path fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" d="M12 3v9M6.3 6.3a8 8 0 1 0 11.4 0"/></svg>';

const powerAction = async (button, path, ask, busy, fallback) => {
  if (!window.confirm(ask)) return;
  button.disabled = true;
  const label = button.textContent;
  button.textContent = busy;
  try {
    const response = await fetch(path, { method: "POST", credentials: "same-origin" });
    const body = await response.json().catch(() => ({}));
    if (response.status === 401) throw new Error("Sign in first, then try again.");
    if (!response.ok) throw new Error(body.message || fallback);
    button.textContent = body.message || busy;
  } catch (error) {
    button.disabled = false;
    button.textContent = label;
    window.alert(String(error.message || error));
  }
};

const signedAs = (who) => {
  if (!who) return null;
  const line = document.createElement("p");
  line.className = "signed-as";
  line.textContent = `Signed in as ${who}`;
  return line;
};

const moreMenu = (groups, who) => {
  const wrap = document.createElement("div");
  wrap.className = "more";
  const toggle = document.createElement("button");
  toggle.type = "button";
  toggle.className = "more-toggle";
  toggle.setAttribute("aria-haspopup", "menu");
  toggle.setAttribute("aria-expanded", "false");
  toggle.textContent = "More";
  const panel = document.createElement("div");
  panel.className = "more-panel";
  panel.setAttribute("role", "menu");
  panel.hidden = true;
  const whoLine = signedAs(who);
  if (whoLine) panel.appendChild(whoLine);
  let current = false;
  groups.forEach((group) => {
    if (!group.items.length) return;
    const block = document.createElement("div");
    block.className = "more-group";
    if (group.title) {
      const title = document.createElement("p");
      title.className = "nav-group-title";
      title.textContent = group.title;
      block.appendChild(title);
    }
    group.items.forEach(([href, label]) => {
      if (isActive(href)) current = true;
      block.appendChild(navLink(href, label, "quiet"));
    });
    panel.appendChild(block);
  });
  if (current) toggle.classList.add("active");
  const close = () => {
    panel.hidden = true;
    toggle.setAttribute("aria-expanded", "false");
    wrap.closest("aside")?.classList.remove("more-open");
  };
  toggle.addEventListener("click", () => {
    panel.hidden = !panel.hidden;
    const open = !panel.hidden;
    toggle.setAttribute("aria-expanded", String(open));
    wrap.closest("aside")?.classList.toggle("more-open", open);
  });
  document.addEventListener("pointerdown", (event) => {
    if (!wrap.contains(event.target)) close();
  });
  wrap.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      close();
      toggle.focus();
    }
  });
  wrap.append(toggle, panel);
  return wrap;
};

const powerMenu = (session) => {
  const wrap = document.createElement("div");
  wrap.className = "power";
  const toggle = document.createElement("button");
  toggle.type = "button";
  toggle.className = "power-toggle";
  toggle.setAttribute("aria-haspopup", "menu");
  toggle.setAttribute("aria-expanded", "false");
  toggle.setAttribute("aria-label", "Power");
  toggle.innerHTML = `${POWER_ICON}<span>Power</span>`;
  const menu = document.createElement("div");
  menu.className = "power-menu";
  menu.setAttribute("role", "menu");
  menu.hidden = true;

  const item = (label, onClick, extra = "") => {
    const button = document.createElement("button");
    button.type = "button";
    button.setAttribute("role", "menuitem");
    button.className = extra;
    button.textContent = label;
    button.addEventListener("click", () => onClick(button));
    menu.appendChild(button);
  };

  if (!session.signed_in) {
    item("Sign in", () => {
      window.location.href = "/login.html";
    });
  } else {
    if (session.can_sign_out) {
      item("Sign out", async () => {
        await fetch("/api/auth/logout", { method: "POST", credentials: "same-origin" }).catch(() => {});
        window.location.href = "/login.html";
      });
    }
    item("Restart", (button) =>
      powerAction(button, "/api/restart", "Restart Keel? Files and apps stop for about a minute, then come back.", "Restarting…", "Restart could not start."),
    );
    item("Shut down", (button) =>
      powerAction(button, "/api/shutdown", "Shut down Keel? You will need to turn the computer on again to use it.", "Shutting down…", "Shutdown could not start."),
    "danger");
  }

  const close = () => {
    menu.hidden = true;
    toggle.setAttribute("aria-expanded", "false");
  };
  toggle.addEventListener("click", () => {
    menu.hidden = !menu.hidden;
    toggle.setAttribute("aria-expanded", String(!menu.hidden));
    if (!menu.hidden) menu.querySelector("button")?.focus();
  });
  document.addEventListener("pointerdown", (event) => {
    if (!wrap.contains(event.target)) close();
  });
  wrap.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      close();
      toggle.focus();
    }
  });
  wrap.append(menu, toggle);
  return wrap;
};

// Pages that need a signed-in person. Signed out, their links go to Sign in
// first, which comes back to the page.
const NEEDS_SIGN_IN = new Set(["/pools.html", "/devices.html", "/installer.html", "/repair.html", "/logs.html", "/settings.html"]);

const installKeelChrome = (state = {}) => {
  const session = { signed_in: Boolean(state.signed_in), can_sign_out: Boolean(state.can_sign_out), who: state.who || "" };
  const nav = document.querySelector("nav");
  const aside = document.querySelector("aside");
  if (!nav || !aside) return;

  const brand = aside.querySelector(".brand");
  if (brand && !brand.querySelector(".brand-mark")) brand.insertAdjacentHTML("afterbegin", BRAND_ICON);

  // The same links every time. Only Install depends on the computer: it
  // means something only on the install USB.
  const computer = computerNav(Boolean(state.can_install));
  nav.replaceChildren(...PRIMARY.map(([href, label]) => navLink(href, label)), computerGroup(computer));
  nav.setAttribute("aria-label", "Everyday");

  aside.querySelectorAll(".sidebar-support, .sidebar-utility, .power, .theme-toggle, .more, .signed-as").forEach((node) => node.remove());
  const supportItems = SUPPORT.filter(([href]) => href !== "/login.html" || !session.signed_in);
  const support = sectionLinks("sidebar-support", "Guide and settings", supportItems);
  const utility = sectionLinks("sidebar-utility", "Notes", NOTES);
  const foot = aside.querySelector(".aside-foot");
  if (foot) {
    aside.insertBefore(support, foot);
    aside.insertBefore(utility, foot);
  } else {
    aside.append(support, utility);
  }
  aside.appendChild(moreMenu([
    { title: "This computer", items: computer },
    { title: "", items: supportItems },
    { title: "", items: NOTES },
  ], session.who));
  const whoLine = signedAs(session.who);
  if (whoLine) aside.appendChild(whoLine);
  aside.appendChild(powerMenu(session));
  if (!session.signed_in) {
    aside.querySelectorAll("a[href]").forEach((link) => {
      const href = link.getAttribute("href");
      if (NEEDS_SIGN_IN.has(href)) link.setAttribute("href", `/login.html?next=${encodeURIComponent(href)}`);
    });
  }
  const themeSlot = document.getElementById("theme-slot");
  if (themeSlot && !themeSlot.querySelector(".theme-toggle")) themeSlot.appendChild(themeControl());
};

// A page that needs a signed-in person says so in words when Keel refuses
// its data, instead of staying empty. Signing in comes back to the page.
const SIGN_IN_PAGES = new Set([...NEEDS_SIGN_IN, "/notes.html", "/sleep.html"]);
const showSignInNeeded = () => {
  if (!SIGN_IN_PAGES.has(window.location.pathname) || document.getElementById("sign-in-needed")) return;
  const main = document.querySelector("main");
  if (!main) return;
  const box = document.createElement("section");
  box.id = "sign-in-needed";
  box.className = "card sign-in-needed";
  const text = document.createElement("p");
  text.textContent = "Sign in to see this page. It shows how this computer is set up, so it opens only for someone signed in.";
  const link = document.createElement("a");
  link.className = "button";
  link.href = `/login.html?next=${encodeURIComponent(window.location.pathname)}`;
  link.textContent = "Sign in";
  box.append(text, link);
  const title = main.querySelector("h1");
  if (title) title.after(box);
  else main.prepend(box);
};
const plainFetch = window.fetch.bind(window);
window.fetch = async (input, init) => {
  const response = await plainFetch(input, init);
  try {
    const url = new URL(typeof input === "string" ? input : input.url, window.location.href);
    const method = ((init && init.method) || (typeof input === "string" ? "GET" : input.method) || "GET").toUpperCase();
    if (response.status === 401 && method === "GET" && url.origin === window.location.origin && url.pathname.startsWith("/api/")) {
      showSignInNeeded();
    }
  } catch { /* the page still gets its answer */ }
  return response;
};

// What Keel said last time, so the sidebar is drawn once, right, instead
// of a guess that changes when the answers come.
const CHROME_KEY = "keel-chrome";
const rememberedChrome = () => {
  try {
    return JSON.parse(window.localStorage.getItem(CHROME_KEY) || "null") || {};
  } catch {
    return {};
  }
};

const bootChrome = () => {
  const before = rememberedChrome();
  installKeelChrome(before);
  const json = (path) =>
    fetch(path, { credentials: "same-origin" }).then((response) => response.json()).catch(() => null);
  Promise.all([json("/api/health"), json("/api/auth/session")]).then(([health, session]) => {
    // Without an answer, keep what is drawn.
    if (!health || !session) return;
    const now = {
      can_install: Boolean(health.can_install),
      signed_in: Boolean(session.signed_in),
      can_sign_out: Boolean(session.can_sign_out),
      who: session.who || "",
    };
    try {
      window.localStorage.setItem(CHROME_KEY, JSON.stringify(now));
    } catch { /* drawn again next time instead */ }
    const same = ["can_install", "signed_in", "can_sign_out", "who"].every((key) => Boolean(before[key]) === Boolean(now[key]) && (key !== "who" || (before.who || "") === now.who));
    if (!same) installKeelChrome(now);
  });
};

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", bootChrome, { once: true });
} else {
  bootChrome();
}

const textBox = (el) => {
  if (!(el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement)) return false;
  if (el.disabled || el.readOnly) return false;
  if (el instanceof HTMLTextAreaElement) return true;
  const type = (el.getAttribute("type") || "text").toLowerCase();
  return ["text", "password", "search", "email", "url", "tel", "number"].includes(type);
};

const screenKeyboard = () => {
  const host = location.hostname.replace(/^\[|\]$/g, "");
  if (host !== "127.0.0.1" && host !== "localhost" && host !== "::1") return;

  const letters = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];
  const numbers = ["1234567890", "-_.:/@", ",?!'+=#"];
  const board = document.createElement("div");
  board.className = "screen-keyboard";
  board.hidden = true;
  board.setAttribute("role", "group");
  board.setAttribute("aria-label", "Keyboard");
  document.body.appendChild(board);

  let target = null;
  let shifted = false;
  let digits = false;

  const caret = (el) => {
    try {
      if (typeof el.selectionStart === "number" && typeof el.selectionEnd === "number") {
        return [el.selectionStart, el.selectionEnd];
      }
    } catch {
      /* A number box has no caret. */
    }
    return [el.value.length, el.value.length];
  };

  const place = (el, pos) => {
    try { el.setSelectionRange(pos, pos); } catch { /* A number box has no caret. */ }
  };

  const write = (text) => {
    if (!target) return;
    const [start, end] = caret(target);
    target.value = target.value.slice(0, start) + text + target.value.slice(end);
    place(target, start + text.length);
    target.dispatchEvent(new Event("input", { bubbles: true }));
  };

  const erase = () => {
    if (!target) return;
    const [start, end] = caret(target);
    if (start !== end) {
      target.value = target.value.slice(0, start) + target.value.slice(end);
      place(target, start);
    } else if (start > 0) {
      target.value = target.value.slice(0, start - 1) + target.value.slice(end);
      place(target, start - 1);
    }
    target.dispatchEvent(new Event("input", { bubbles: true }));
  };

  const key = (label, className, press) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = className ? `screen-key ${className}` : "screen-key";
    button.textContent = label;
    button.addEventListener("pointerdown", (event) => {
      event.preventDefault();
      event.stopPropagation();
      press();
    });
    return button;
  };

  const row = (...buttons) => {
    const line = document.createElement("div");
    line.className = "screen-keyboard-row";
    buttons.forEach((button) => line.appendChild(button));
    return line;
  };

  const finish = () => {
    const el = target;
    target = null;
    shifted = false;
    digits = false;
    board.hidden = true;
    document.body.classList.remove("screen-keyboard-open");
    el?.blur();
  };

  const draw = () => {
    board.replaceChildren();
    if (digits) {
      board.append(
        row(...numbers[0].split("").map((char) => key(char, "", () => write(char)))),
        row(...numbers[1].split("").map((char) => key(char, "", () => write(char)))),
        row(...numbers[2].split("").map((char) => key(char, "", () => write(char)))),
        row(
          key("Letters", "wide", () => { digits = false; draw(); }),
          key("Space", "space", () => write(" ")),
          key("Delete", "wide", erase),
          key("Done", "wide", finish),
        ),
      );
      return;
    }
    letters.forEach((line, index) => {
      const buttons = line.split("").map((char) => {
        const shown = shifted ? char.toUpperCase() : char;
        return key(shown, "", () => {
          write(shown);
          if (shifted) {
            shifted = false;
            draw();
          }
        });
      });
      if (index === 2) {
        const shift = key("Shift", "wide", () => { shifted = !shifted; draw(); });
        shift.setAttribute("aria-pressed", String(shifted));
        buttons.unshift(shift);
        buttons.push(key("Delete", "wide", erase));
      }
      board.appendChild(row(...buttons));
    });
    board.appendChild(row(
      key("Numbers", "wide", () => { digits = true; shifted = false; draw(); }),
      key("Space", "space", () => write(" ")),
      key("Done", "wide", finish),
    ));
  };

  const lift = (el) => {
    const rect = el.getBoundingClientRect();
    const limit = window.innerHeight - board.offsetHeight - 16;
    if (rect.bottom <= limit) return;
    const scroller = el.closest(".scroll") || document.querySelector("main");
    if (scroller) scroller.scrollTop += rect.bottom - limit;
  };

  const show = (el) => {
    if (target !== el) {
      target = el;
      shifted = false;
      digits = false;
    }
    board.hidden = false;
    document.body.classList.add("screen-keyboard-open");
    draw();
    requestAnimationFrame(() => lift(el));
  };

  board.addEventListener("pointerdown", (event) => event.preventDefault(), true);

  const boxFrom = (node) => {
    if (textBox(node)) return node;
    if (!(node instanceof Element)) return null;
    const label = node.closest("label");
    if (!label) return null;
    return [...label.querySelectorAll("input, textarea")].find(textBox) || null;
  };

  document.addEventListener("focusin", (event) => {
    if (board.contains(event.target)) return;
    const box = boxFrom(event.target);
    if (box) show(box);
  });

  document.addEventListener("pointerdown", (event) => {
    const box = boxFrom(event.target);
    if (box) {
      show(box);
      return;
    }
    if (board.hidden) return;
    const node = event.target;
    if (!(node instanceof Node) || board.contains(node)) return;
    finish();
  });

  draw();
  if (textBox(document.activeElement)) show(document.activeElement);
};

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", screenKeyboard, { once: true });
} else {
  screenKeyboard();
}

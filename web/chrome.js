const PRIMARY = [
  ["/", "Home"],
  ["/shares.html", "Files"],
  ["/catalog.html", "Apps"],
  ["/overlay.html", "Away"],
];
const computerNav = (showInstall) => {
  const items = [["/pools.html", "Disks"]];
  if (showInstall) items.push(["/installer.html", "Install"]);
  items.push(["/repair.html", "Repair"]);
  return items;
};
const SUPPORT = [
  ["/guide.html", "Guide"],
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

const installKeelChrome = (showInstall = false, session = { signed_in: false, can_sign_out: false }, showApps = false) => {
  const nav = document.querySelector("nav");
  const aside = document.querySelector("aside");
  if (!nav || !aside) return;

  const brand = aside.querySelector(".brand");
  if (brand && !brand.querySelector(".brand-mark")) brand.insertAdjacentHTML("afterbegin", BRAND_ICON);

  const everyday = PRIMARY.filter(([href]) => href !== "/catalog.html" || showApps || isActive(href));
  const computer = computerNav(showInstall);
  const showComputer = session.signed_in || computer.some(([href]) => isActive(href));
  const children = everyday.map(([href, label]) => navLink(href, label));
  if (showComputer) children.push(computerGroup(computer));
  nav.replaceChildren(...children);
  nav.setAttribute("aria-label", "Everyday");

  aside.querySelectorAll(".sidebar-support, .sidebar-utility, .power, .theme-toggle").forEach((node) => node.remove());
  const supportItems = SUPPORT.filter(([href]) => {
    if (href === "/login.html") return !session.signed_in || isActive(href);
    if (href === "/settings.html") return session.signed_in || isActive(href);
    return true;
  });
  const support = sectionLinks("sidebar-support", "Guide and settings", supportItems);
  const utility = sectionLinks("sidebar-utility", "Notes", NOTES);
  const foot = aside.querySelector(".aside-foot");
  if (foot) {
    aside.insertBefore(support, foot);
    aside.insertBefore(utility, foot);
  } else {
    aside.append(support, utility);
  }
  aside.appendChild(powerMenu(session));
  aside.appendChild(themeControl());
};

const bootChrome = () => {
  installKeelChrome(false);
  const json = (path) =>
    fetch(path, { credentials: "same-origin" }).then((response) => response.json()).catch(() => ({}));
  Promise.all([json("/api/health"), json("/api/auth/session"), json("/api/apps")]).then(([health, session, apps]) => {
    const installed = Array.isArray(apps) && apps.some((app) => app.installed);
    installKeelChrome(Boolean(health.can_install), {
      signed_in: Boolean(session.signed_in),
      can_sign_out: Boolean(session.can_sign_out),
    }, installed);
  });
};

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", bootChrome, { once: true });
} else {
  bootChrome();
}

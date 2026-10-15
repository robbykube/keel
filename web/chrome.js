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

let flyoutChromeInstalled = false;

const currentPath = () => location.pathname || "/";

const isActive = (href) => {
  const current = currentPath();
  if (href === "/") return current === "/" || current === "/index.html";
  return current === href;
};

const navLink = (href, label, extraClass = "") => {
  const link = document.createElement("a");
  link.href = href;
  link.textContent = label;
  link.className = [extraClass, isActive(href) ? "active" : ""].filter(Boolean).join(" ");
  if (isActive(href)) link.setAttribute("aria-current", "page");
  return link;
};

const closeFlyout = (flyout) => {
  flyout.classList.remove("is-open");
  flyout.querySelector(".nav-flyout-toggle")?.setAttribute("aria-expanded", "false");
};

const installFlyoutHandlers = (root) => {
  root.querySelectorAll(".nav-flyout").forEach((flyout) => {
    const toggle = flyout.querySelector(".nav-flyout-toggle");
    const menu = flyout.querySelector(".nav-flyout-menu");
    if (!toggle || !menu) return;
    const items = () => [...menu.querySelectorAll("a")];
    let closeTimer = 0;
    const open = () => {
      window.clearTimeout(closeTimer);
      document.querySelectorAll(".nav-flyout.is-open").forEach((other) => {
        if (other !== flyout) closeFlyout(other);
      });
      flyout.classList.add("is-open");
      toggle.setAttribute("aria-expanded", "true");
      const row = toggle.getBoundingClientRect();
      const asideBox = flyout.closest("aside")?.getBoundingClientRect();
      const mobile = window.matchMedia("(max-width: 960px)").matches;
      if (mobile) {
        menu.style.left = `${Math.max(8, row.left)}px`;
        menu.style.top = `${row.bottom + 4}px`;
      } else {
        menu.style.left = `${(asideBox?.right ?? row.right) + 4}px`;
        menu.style.top = `${Math.max(8, Math.min(row.top - 7, window.innerHeight - menu.offsetHeight - 8))}px`;
      }
    };
    flyout.addEventListener("mouseenter", open);
    flyout.addEventListener("mouseleave", () => {
      closeTimer = window.setTimeout(() => closeFlyout(flyout), 220);
    });
    toggle.addEventListener("click", (event) => {
      open();
      if (event.detail === 0) {
        (items().find((item) => item.classList.contains("active")) ?? items()[0])?.focus();
      }
    });
    menu.addEventListener("keydown", (event) => {
      if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
      event.preventDefault();
      const list = items();
      const at = list.indexOf(document.activeElement);
      list[(at + (event.key === "ArrowDown" ? 1 : list.length - 1) + list.length) % list.length]?.focus();
    });
    flyout.addEventListener("focusout", (event) => {
      if (!flyout.contains(event.relatedTarget)) closeFlyout(flyout);
    });
  });
};

const installFlyoutChrome = () => {
  if (flyoutChromeInstalled) return;
  flyoutChromeInstalled = true;
  document.addEventListener("pointerdown", (event) => {
    document.querySelectorAll(".nav-flyout.is-open").forEach((flyout) => {
      if (!flyout.contains(event.target)) closeFlyout(flyout);
    });
  });
  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    document.querySelectorAll(".nav-flyout.is-open").forEach((flyout) => {
      const hadFocus = flyout.contains(document.activeElement);
      closeFlyout(flyout);
      if (hadFocus) flyout.querySelector(".nav-flyout-toggle")?.focus();
    });
  });
};

const navFlyout = (label, items) => {
  const wrap = document.createElement("div");
  wrap.className = "nav-flyout";
  const current = items.find(([href]) => isActive(href));
  const toggle = document.createElement("button");
  toggle.type = "button";
  toggle.className = `nav-flyout-toggle${current ? " active" : ""}`;
  toggle.setAttribute("aria-haspopup", "menu");
  toggle.setAttribute("aria-expanded", "false");
  const name = document.createElement("span");
  name.textContent = label;
  toggle.appendChild(name);
  if (current) {
    const small = document.createElement("small");
    small.textContent = current[1];
    toggle.appendChild(small);
  }
  const menu = document.createElement("div");
  menu.className = "nav-flyout-menu";
  menu.setAttribute("role", "menu");
  menu.setAttribute("aria-label", label);
  items.forEach(([href, name]) => {
    const link = navLink(href, name);
    link.setAttribute("role", "menuitem");
    menu.appendChild(link);
  });
  wrap.append(toggle, menu);
  return wrap;
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

const installKeelChrome = (showInstall = false, session = { signed_in: false, can_sign_out: false }) => {
  const nav = document.querySelector("nav");
  const aside = document.querySelector("aside");
  if (!nav || !aside) return;

  const flyouts = document.createElement("div");
  flyouts.className = "nav-flyouts";
  flyouts.appendChild(navFlyout("This computer", computerNav(showInstall)));
  nav.replaceChildren(
    ...PRIMARY.map(([href, label]) => navLink(href, label)),
    flyouts,
  );
  nav.setAttribute("aria-label", "Everyday");

  aside.querySelectorAll(".sidebar-support, .sidebar-utility, .power").forEach((node) => node.remove());
  const supportItems = session.signed_in
    ? SUPPORT.filter(([href]) => href !== "/login.html" || isActive(href))
    : SUPPORT;
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
  installFlyoutChrome();
  installFlyoutHandlers(aside);
};

const bootChrome = () => {
  installKeelChrome(false);
  const json = (path) =>
    fetch(path, { credentials: "same-origin" }).then((response) => response.json()).catch(() => ({}));
  Promise.all([json("/api/health"), json("/api/auth/session")]).then(([health, session]) => {
    installKeelChrome(Boolean(health.can_install), {
      signed_in: Boolean(session.signed_in),
      can_sign_out: Boolean(session.can_sign_out),
    });
  });
};

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", bootChrome, { once: true });
} else {
  bootChrome();
}

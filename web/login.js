const form = document.getElementById("form");
const token = document.getElementById("token");
const status = document.getElementById("status");
const lede = document.getElementById("lede");
const showToken = document.getElementById("show-token");
const localLogin = document.getElementById("local-login");
const askKeel = document.getElementById("ask-keel");
const onThisComputer = ["127.0.0.1", "localhost", "::1"].includes(window.location.hostname);

const afterSignIn = () => {
  const next = new URLSearchParams(location.search).get("next") || "/";
  return /^\/[A-Za-z0-9._~/-]*$/.test(next) ? next : "/";
};
let waiting = null;

if (onThisComputer) {
  askKeel.hidden = true;
  localLogin.hidden = false;
  lede.textContent = "This is the Keel computer. When another computer asks to sign in, press Allow on the notice.";
  localLogin.addEventListener("click", () => { window.location.href = "/"; });
}

showToken.addEventListener("click", () => {
  const visible = token.type === "text";
  token.type = visible ? "password" : "text";
  showToken.textContent = visible ? "Show" : "Hide";
});

const stopWaiting = (message) => {
  waiting = null;
  askKeel.disabled = false;
  askKeel.textContent = "Ask the Keel computer";
  status.textContent = message;
};

askKeel.addEventListener("click", async () => {
  if (waiting) {
    waiting.cancelled = true;
    stopWaiting("Cancelled. Ask again when someone is at the Keel computer.");
    return;
  }
  const ticket = { cancelled: false };
  waiting = ticket;
  askKeel.textContent = "Cancel";
  status.textContent = "Waiting for someone at the Keel computer to press Allow.";
  try {
    const res = await fetch("/api/auth/requests", { method: "POST", credentials: "same-origin" });
    const body = await res.text();
    if (!res.ok) throw new Error(body || "Could not ask the Keel computer. Try again.");
    const id = JSON.parse(body).id;
    const started = Date.now();
    while (Date.now() - started < 130000) {
      if (ticket.cancelled) return;
      await new Promise((resolve) => setTimeout(resolve, 1000));
      if (ticket.cancelled) return;
      const poll = await fetch(`/api/auth/requests/${id}`, { credentials: "same-origin" });
      const view = await poll.json().catch(() => ({}));
      if (view.state === "allowed") {
        window.location.href = afterSignIn();
        return;
      }
      if (view.state === "denied") {
        stopWaiting("The Keel computer did not allow this browser. Ask again, or use the admin token.");
        return;
      }
      if (view.state === "expired" || !poll.ok) {
        stopWaiting("That request expired. Ask the Keel computer again.");
        return;
      }
    }
    stopWaiting("That request expired. Ask the Keel computer again.");
  } catch (err) {
    stopWaiting(String(err.message || err));
  }
});

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!token.value.trim()) {
    status.textContent = "Type the admin token, or ask the Keel computer.";
    return;
  }
  status.textContent = "Signing in.";
  try {
    const res = await fetch("/api/auth/login", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify({ token: token.value }),
    });
    const data = await res.text();
    if (!res.ok) throw new Error(data || "Sign-in failed. Ask the Keel computer, or type the token again.");
    window.location.href = afterSignIn();
  } catch (err) {
    status.textContent = String(err.message || err);
  }
});

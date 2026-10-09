const form = document.getElementById("form");
const token = document.getElementById("token");
const status = document.getElementById("status");

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  status.textContent = "signing in…";
  try {
    const res = await fetch("/api/auth/login", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify({ token: token.value }),
    });
    const data = await res.text();
    if (!res.ok) throw new Error(data || "sign-in failed");
    window.location.href = "/";
  } catch (err) {
    status.textContent = String(err.message || err);
  }
});

// Tells the page whether it is running on a real Keel appliance or on a
// developer's Windows copy, so text written for one never shows on the other.
//
// Mark elements with data-only="linux" or data-only="windows". They start
// hidden in style.css and only the matching ones are revealed, so a wrong
// message is never briefly visible while this request is in flight.
(async () => {
  const fallback = () => {
    // If /api/system cannot be reached, show the appliance wording. A real
    // appliance that briefly looks like a dev host is worse than the reverse:
    // the dev host is only ever seen by us.
    document.documentElement.dataset.platform = "linux";
  };

  try {
    const res = await fetch("/api/system");
    if (!res.ok) return fallback();
    const data = await res.json();
    document.documentElement.dataset.platform =
      data.platform === "windows" ? "windows" : "linux";
  } catch {
    fallback();
  }
})();

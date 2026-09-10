/* Short forms → Guide. Longest labels first so "OpenCloud" beats "Cloud". */
const KEEL_TERMS = [
  ["OpenCloud", "opencloud"],
  ["Tailscale", "tailscale"],
  ["systemctl", "systemctl"],
  ["TrueNAS", "truenas"],
  ["Proxmox", "proxmox"],
  ["lsblk", "lsblk"],
  ["SMART", "smart"],
  ["Samba", "share"],
  ["RSS", "rss"],
  ["SMB", "share"],
  ["LXC", "lxc"],
  ["nft", "nft"],
  ["API", "api"],
  ["Away", "away"],
];

function keelLinkTerms(text) {
  if (!text) return "";
  let out = String(text);
  for (const [label, id] of KEEL_TERMS) {
    const re = new RegExp(`\\b(${label.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})\\b`, "g");
    out = out.replace(re, `<a class="term" href="/guide.html#${id}">$1</a>`);
  }
  return out;
}

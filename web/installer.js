async function loadInstaller() {
  const target = document.querySelector('#installer-status');
  try {
    const response = await fetch('/api/installer');
    const info = await response.json();
    const disks = (info.disks || []).map(d => `${d.name} — ${d.model || 'unknown model'} — ${formatBytes(d.size)}`).join('<br>');
    target.innerHTML = `<p><strong>Mode:</strong> ${info.mode}</p><p>${info.warning}</p><p><strong>Disks detected:</strong><br>${disks || 'No disks reported yet.'}</p><p><strong>Network:</strong> ${info.network}</p>`;
  } catch (_) {
    target.textContent = 'Keel could not read installer status. Try the Guide or use recovery mode.';
  }
}

function formatBytes(value) {
  if (!value) return 'size unavailable';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let size = value;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) { size /= 1024; unit += 1; }
  return `${size.toFixed(unit ? 1 : 0)} ${units[unit]}`;
}

loadInstaller();

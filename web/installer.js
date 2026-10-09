async function loadInstaller() {
  const target = document.querySelector('#installer-status');
  try {
    const response = await fetch('/api/installer');
    const info = await response.json();
    const disks = (info.disks || []).map(d => `${d.name} — ${d.model || 'unknown model'} — ${formatBytes(d.size)}`).join('<br>');
    target.innerHTML = `<p><strong>Mode:</strong> ${info.mode}</p><p>${info.warning}</p><p><strong>Disks detected:</strong><br>${disks || 'No disks reported yet.'}</p><p><strong>Network:</strong> ${info.network}</p>`;
    const form = document.querySelector('#install-form');
    if (info.can_install && info.disks?.length) {
      form.hidden = false;
      const select = document.querySelector('#install-target');
      select.innerHTML = info.disks.map(d => `<option value="${d.path}">${d.name} — ${d.model || 'unknown model'} — ${formatBytes(d.size)}</option>`).join('');
      document.querySelector('#install-button').addEventListener('click', installKeel);
    }
  } catch (_) {
    target.textContent = 'Keel could not read installer status. Try the Guide or use recovery mode.';
  }
}

async function installKeel() {
  const result = document.querySelector('#install-result');
  result.textContent = 'Checking the selected disk…';
  const response = await fetch('/api/installer/plan', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ target: document.querySelector('#install-target').value, confirmation: document.querySelector('#install-confirm').value }),
  });
  const body = await response.json().catch(() => ({}));
  result.textContent = body.message || 'Installation could not start.';
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

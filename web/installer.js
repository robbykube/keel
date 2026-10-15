let pollTimer = 0;
let installBound = false;

async function loadInstaller() {
  const target = document.querySelector('#installer-status');
  try {
    const health = await fetch('/api/health', { credentials: 'same-origin' }).then((r) => r.json());
    if (!health.can_install) {
      location.replace('/');
      return;
    }
    const response = await fetch('/api/installer');
    const info = await response.json();
    const disks = (info.disks || []).map(d => `${d.name} — ${d.model || 'unknown model'} — ${formatBytes(d.size)}`).join('<br>');
    target.innerHTML = `<p><strong>Mode:</strong> ${info.mode}</p><p>${info.warning}</p><p><strong>Disks detected:</strong><br>${disks || 'No disks reported yet.'}</p><p><strong>Network:</strong> ${info.network}</p>`;
    const form = document.querySelector('#install-form');
    if (info.can_install && info.disks?.length) {
      form.hidden = false;
      const select = document.querySelector('#install-target');
      select.innerHTML = info.disks.map(d => `<option value="${d.path}">${d.name} — ${d.model || 'unknown model'} — ${formatBytes(d.size)}</option>`).join('');
      if (!installBound) {
        document.querySelector('#install-button').addEventListener('click', installKeel);
        installBound = true;
      }
    }
    await refreshProgress();
  } catch (_) {
    target.textContent = 'Keel could not read installer status. Try the Guide or use recovery mode.';
  }
}

async function installKeel() {
  const button = document.querySelector('#install-button');
  const result = document.querySelector('#install-result');
  button.disabled = true;
  result.textContent = '';
  showProgress({ state: 'running', percent: 1, message: 'Starting the install.' });
  try {
    const response = await fetch('/api/installer/plan', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ target: document.querySelector('#install-target').value, confirmation: document.querySelector('#install-confirm').value }),
    });
    const raw = await response.text();
    let message = raw || 'Installation could not start.';
    try { message = JSON.parse(raw).message || message; } catch (_) { /* auth errors are plain text */ }
    if (response.status === 401 || response.status === 403) {
      hideProgress();
      result.innerHTML = `${message} <a href="/login.html">Sign in, then try again.</a>`;
      button.disabled = false;
      return;
    }
    if (!response.ok) {
      hideProgress();
      result.textContent = message;
      button.disabled = false;
      return;
    }
    await refreshProgress();
  } catch (error) {
    hideProgress();
    result.textContent = `Installation could not start: ${error.message || error}`;
    button.disabled = false;
  }
}

async function refreshProgress() {
  try {
    const response = await fetch('/api/installer/progress', { credentials: 'same-origin' });
    if (!response.ok) return;
    const progress = await response.json();
    if (progress.state === 'running' || progress.state === 'done' || progress.state === 'error') {
      showProgress(progress);
    }
    if (progress.state === 'running') {
      schedulePoll();
    } else {
      stopPoll();
      const button = document.querySelector('#install-button');
      if (button && progress.state !== 'done') button.disabled = false;
    }
  } catch (_) {
    schedulePoll();
  }
}

function showProgress(progress) {
  const form = document.querySelector('#install-form');
  const panel = document.querySelector('#install-progress');
  const message = document.querySelector('#install-message');
  const bar = document.querySelector('#install-bar');
  const fill = bar.querySelector('span');
  const done = document.querySelector('#install-done');
  const token = document.querySelector('#install-token');
  const result = document.querySelector('#install-result');
  panel.hidden = false;
  message.textContent = progress.message || '';
  const percent = Math.max(0, Math.min(100, Number(progress.percent) || 0));
  fill.style.width = `${percent}%`;
  bar.setAttribute('aria-valuenow', String(percent));
  bar.hidden = progress.state === 'error';
  if (progress.state === 'running') {
    form.hidden = true;
    done.hidden = true;
    result.textContent = '';
  } else if (progress.state === 'done') {
    form.hidden = true;
    fill.style.width = '100%';
    bar.setAttribute('aria-valuenow', '100');
    if (progress.token) {
      token.textContent = progress.token;
      done.hidden = false;
    } else {
      done.hidden = true;
    }
  } else if (progress.state === 'error') {
    form.hidden = false;
    done.hidden = true;
    result.textContent = '';
  }
}

function hideProgress() {
  document.querySelector('#install-progress').hidden = true;
}

function schedulePoll() {
  if (pollTimer) return;
  pollTimer = window.setTimeout(() => {
    pollTimer = 0;
    refreshProgress();
  }, 400);
}

function stopPoll() {
  if (pollTimer) {
    window.clearTimeout(pollTimer);
    pollTimer = 0;
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

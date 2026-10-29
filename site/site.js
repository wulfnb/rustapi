const copyButton = document.querySelector('#copy-docker');
const copyStatus = document.querySelector('#copy-status');
if (copyButton) {
  copyButton.addEventListener('click', async () => {
    const command = "docker run -d --name rustapi-client -p 127.0.0.1:7443:7443 -e API_USER=tester -e API_PASSWORD='choose-a-strong-password' -v rustapi-data:/config ghcr.io/wulfnb/rustapi:latest";
    try {
      if (!navigator.clipboard?.writeText) throw new Error('Clipboard unavailable');
      await navigator.clipboard.writeText(command);
      copyStatus.textContent = 'Docker command copied. Replace the example password before running.';
      copyButton.textContent = 'Copied!';
    } catch {
      copyButton.textContent = 'Copy';
      copyStatus.textContent = 'Could not copy automatically. Select and copy the command below.';
      const range = document.createRange();
      range.selectNodeContents(document.querySelector('#docker-command'));
      const selection = window.getSelection();
      selection.removeAllRanges();
      selection.addRange(range);
      document.querySelector('#docker-command').focus();
    }
  });
}

document.querySelectorAll('.mobile-menu a').forEach(link => {
  link.addEventListener('click', () => link.closest('details')?.removeAttribute('open'));
});

// Highlight user platform automatically if detected
(function detectPlatform() {
  const ua = navigator.userAgent || '';
  let id = null;
  if (/Macintosh|Mac OS X/i.test(ua)) {
    id = '#download-mac-arm';
  } else if (/Windows/i.test(ua)) {
    id = '#download-win-exe';
  } else if (/Linux/i.test(ua)) {
    id = '#download-linux';
  }
  if (id) {
    const el = document.querySelector(id);
    if (el) {
      el.classList.add('recommended-download');
      const badge = document.createElement('span');
      badge.className = 'pill-recommended';
      badge.textContent = 'Recommended for you';
      el.prepend(badge);
    }
  }
})();

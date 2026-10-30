const copyButton = document.querySelector('#copy-docker');
const copyStatus = document.querySelector('#copy-status');
if (copyButton) {
  copyButton.addEventListener('click', async () => {
    const command = "docker run -d --name rustapi-client -p 127.0.0.1:6080:6080 -v rustapi-data:/data ghcr.io/wulfnb/rustapi:edge";
    try {
      if (!navigator.clipboard?.writeText) throw new Error('Clipboard unavailable');
      await navigator.clipboard.writeText(command);
      copyStatus.textContent = 'Docker command copied. Open http://localhost:6080/vnc.html?autoconnect=true&resize=scale after starting the container.';
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

// GitHub's /releases/latest excludes previews. Resolve a complete published
// release, keeping the pinned HTML links usable without JavaScript or API access.
async function refreshDownloads() {
  const base = 'https://github.com/wulfnb/rustapi/releases';
  const required = [
    'RustAPI-macos-arm64.dmg', 'RustAPI-macos-arm64.zip',
    'RustAPI-macos-x86_64.dmg', 'RustAPI-macos-x86_64.zip',
    'RustAPI-windows-x86_64-Setup.exe', 'RustAPI-windows-x86_64.zip',
    'RustAPI-linux-x86_64.tar.gz', 'SHA256SUMS.txt'
  ];
  try {
    const response = await fetch('https://api.github.com/repos/wulfnb/rustapi/releases?per_page=100', {
      headers: {Accept: 'application/vnd.github+json'},
      signal: AbortSignal.timeout(8000)
    });
    if (!response.ok) return;
    const releases = await response.json();
    if (!Array.isArray(releases)) return;
    const complete = releases.filter(release => {
      if (release.draft || !release.published_at || !Array.isArray(release.assets)) return false;
      return required.every(name => release.assets.some(asset =>
        asset.name === name && asset.state === 'uploaded' && asset.size > 0 &&
        asset.browser_download_url === `${base}/download/${encodeURIComponent(release.tag_name)}/${name}`
      ));
    }).sort((a, b) => Date.parse(b.published_at) - Date.parse(a.published_at));
    if (!complete.length) return;
    const release = complete[0];
    document.querySelectorAll('[data-release-asset]').forEach(link => {
      if (required.includes(link.dataset.releaseAsset)) {
        link.href = `${base}/download/${encodeURIComponent(release.tag_name)}/${link.dataset.releaseAsset}`;
      }
    });
    document.querySelector('#release-status').textContent =
      `${release.prerelease ? 'Preview' : 'Release'} ${release.tag_name}. All downloads and checksums are from this build.`;
  } catch {
    // Offline, rate-limited, or unavailable API: retain the complete pinned build.
  }
}

refreshDownloads();

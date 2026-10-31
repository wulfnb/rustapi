const pair = document.getElementById('pair');
const status = document.getElementById('status');
const saved = await chrome.storage.session.get('pair');
pair.value = saved.pair || '';
function connection(value) {
  const url = new URL(value);
  if (url.protocol !== 'http:' || url.hostname !== '127.0.0.1' || !url.port || url.username || url.password || !/^#[a-f0-9]{64}$/.test(url.hash)) throw new Error('Copy a current private pairing URL from RustAPI.');
  return { base: url.origin, token: url.hash.slice(1) };
}
async function transfer(direction) {
  try {
    status.textContent = 'Connecting…';
    const { base, token } = connection(pair.value);
    const [tab] = await chrome.tabs.query({active: true, currentWindow: true});
    const url = new URL(tab.url);
    if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) throw new Error('Open an HTTP(S) tab first.');
    if (!await chrome.permissions.request({origins: [`${url.protocol}//${url.hostname}/*`]})) throw new Error('Host permission was declined.');
    await chrome.storage.session.set({pair: pair.value});
    const headers = {Authorization: `Bearer ${token}`, 'Content-Type': 'application/json'};
    let count = 0, skipped = 0;
    if (direction === 'capture') {
      const all = await chrome.cookies.getAll({domain: url.hostname});
      const matching = all.filter(c => c.hostOnly ? c.domain === url.hostname : (url.hostname === c.domain.replace(/^\./, '') || url.hostname.endsWith(`.${c.domain.replace(/^\./, '')}`)));
      skipped = matching.filter(c => c.partitionKey).length;
      const cookies = matching.filter(c => !c.partitionKey);
      const response = await fetch(`${base}/cookies`, {method: 'POST', headers, body: JSON.stringify({origin: url.origin, cookies}), signal: AbortSignal.timeout(10000)});
      if (!response.ok) throw new Error(`Capture failed (${response.status}). Check pairing, origin and cookie attributes.`);
      count = (await response.json()).count;
    } else {
      const response = await fetch(`${base}/cookies?origin=${encodeURIComponent(url.origin)}`, {headers, signal: AbortSignal.timeout(10000)});
      if (!response.ok) throw new Error(`Sync failed (${response.status}). Enable the connection and pair again.`);
      const {cookies} = await response.json();
      for (const cookie of cookies) {
        const details = {url: `${cookie.secure ? 'https:' : url.protocol}//${url.host}${cookie.path}`, name: cookie.name, value: cookie.value, path: cookie.path, secure: cookie.secure, httpOnly: cookie.httpOnly};
        if (!cookie.hostOnly) details.domain = cookie.domain;
        if (cookie.expirationDate !== null) details.expirationDate = cookie.expirationDate;
        if (cookie.sameSite) details.sameSite = cookie.sameSite === 'none' ? 'no_restriction' : cookie.sameSite;
        const result = await chrome.cookies.set(details);
        if (!result) throw new Error(`Browser rejected cookie ${cookie.name}.`);
        count++;
      }
    }
    status.textContent = `${count} cookies ${direction === 'capture' ? 'captured' : 'merged'}.${skipped ? ` ${skipped} partitioned cookies skipped.` : ''}`;
  } catch (error) {status.textContent = error.message;}
}
document.getElementById('capture').addEventListener('click', () => transfer('capture'));
document.getElementById('sync').addEventListener('click', () => transfer('sync'));

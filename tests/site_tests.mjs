import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {fileURLToPath} from 'node:url';

const filename = fileURLToPath(new URL('../site/site.js', import.meta.url));

function fixture(clipboard, userAgent = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)', fetchReleases = async () => ({ok: false})) {
  let copy, menuClick;
  const button = {textContent: 'Copy', addEventListener: (_, fn) => { copy = fn; }};
  const notice = {textContent: ''};
  const releaseStatus = {textContent: 'Pinned preview'};
  const downloads = ['RustAPI-macos-arm64.dmg', 'RustAPI-windows-x86_64-Setup.exe', 'SHA256SUMS.txt']
    .map(name => ({dataset: {releaseAsset: name}, href: 'pinned/' + name}));
  let selected = false, focused = false, closed = false;
  const command = {focus: () => { focused = true; }};
  const link = {
    addEventListener: (_, fn) => { menuClick = fn; },
    closest: () => ({removeAttribute: () => { closed = true; }})
  };
  const recElement = {
    classList: {add: () => {}},
    prepend: () => {}
  };
  const context = {
    fetch: fetchReleases,
    AbortSignal,
    navigator: {clipboard, userAgent},
    document: {
      querySelector: s => {
        if (s === '#copy-docker') return button;
        if (s === '#copy-status') return notice;
        if (s === '#release-status') return releaseStatus;
        if (s.startsWith('#download-')) return recElement;
        return command;
      },
      querySelectorAll: s => s === '[data-release-asset]' ? downloads : [link],
      createElement: () => ({className: '', textContent: ''}),
      createRange: () => ({selectNodeContents: node => assert.equal(node, command)})
    },
    window: {
      getSelection: () => ({removeAllRanges() {}, addRange() { selected = true; }})
    }
  };
  const refresh = new vm.Script(readFileSync(filename, 'utf8'), {filename}).runInNewContext(context);
  return {button, notice, copy, menuClick, downloads, releaseStatus, refresh, state: () => ({selected, focused, closed})};
}

test('website Copy announces success after clipboard operation resolves', async () => {
  let finish, copied;
  const f = fixture({
    writeText: text => {
      copied = text;
      return new Promise(resolve => { finish = resolve; });
    }
  });
  const pending = f.copy();
  assert.equal(f.button.textContent, 'Copy');
  assert.equal(f.notice.textContent, '');
  finish();
  await pending;
  assert.equal(f.button.textContent, 'Copied!');
  assert.match(copied, /127\.0\.0\.1:6080:6080/);
  assert.match(copied, /rustapi-data:\/data/);
  assert.match(copied, /ghcr.io\/wulfnb\/rustapi:edge/);
  f.menuClick();
  assert.equal(f.state().closed, true);
});

for (const [name, clipboard] of [['unavailable', undefined], ['denied', {writeText: async () => { throw Error('denied'); }}]]) {
  test('website Copy selects readable fallback when ' + name, async () => {
    const f = fixture(clipboard);
    await f.copy();
    assert.equal(f.button.textContent, 'Copy');
    assert.match(f.notice.textContent, /Could not copy/);
    assert.equal(f.state().focused, true);
    assert.equal(f.state().selected, true);
  });
}

const assets = [
  'RustAPI-macos-arm64.dmg', 'RustAPI-macos-arm64.zip',
  'RustAPI-macos-x86_64.dmg', 'RustAPI-macos-x86_64.zip',
  'RustAPI-windows-x86_64-Setup.exe', 'RustAPI-windows-x86_64.zip',
  'RustAPI-linux-x86_64.tar.gz', 'SHA256SUMS.txt'
];

function release(tag, date, overrides = {}) {
  return {
    tag_name: tag, published_at: date, draft: false, prerelease: true,
    assets: assets.map(name => ({name, state: 'uploaded', size: 100,
      browser_download_url: `https://github.com/wulfnb/rustapi/releases/download/${tag}/${name}`})),
    ...overrides
  };
}

test('downloads select the newest complete published preview and keep checksums on that build', async () => {
  const incomplete = release('preview-new', '2026-10-30T12:00:00Z');
  incomplete.assets = incomplete.assets.filter(a => !a.name.endsWith('-Setup.exe'));
  const selected = release('preview-complete', '2026-10-29T12:00:00Z');
  const f = fixture(undefined, 'Linux', async (url, options) => {
    assert.equal(url, 'https://api.github.com/repos/wulfnb/rustapi/releases?per_page=100');
    assert.ok(options.signal);
    return {ok: true, json: async () => [
      release('preview-old', '2026-10-28T12:00:00Z'), incomplete,
      release('draft', '2026-10-31T12:00:00Z', {draft: true}), selected
    ]};
  });
  await f.refresh;
  for (const link of f.downloads) assert.match(link.href, /\/preview-complete\//);
  assert.match(f.releaseStatus.textContent, /Preview preview-complete/);
});

test('stable releases and previews share the same completeness checks', async () => {
  const f = fixture(undefined, 'Windows', async () => ({ok: true, json: async () => [
    release('v0.2.0', '2026-10-30T12:00:00Z', {prerelease: false})
  ]}));
  await f.refresh;
  assert.match(f.releaseStatus.textContent, /^Release v0.2.0/);
  for (const link of f.downloads) assert.match(link.href, /\/v0\.2\.0\//);
});

for (const [name, fetchReleases] of [
  ['API rate limit', async () => ({ok: false})],
  ['network failure', async () => { throw Error('offline'); }],
  ['malformed response', async () => ({ok: true, json: async () => ({message: 'unavailable'})})],
  ['no releases', async () => ({ok: true, json: async () => []})],
  ['unpublished or malformed release', async () => ({ok: true, json: async () => [
    release('unpublished', null), release('malformed', '2026-10-30', {assets: null})
  ]})],
  ['unsafe or incomplete assets', async () => {
    const unsafe = release('unsafe', '2026-10-30');
    unsafe.assets[0].browser_download_url = 'https://example.com/installer';
    const empty = release('empty', '2026-10-30');
    empty.assets[0].size = 0;
    const uploading = release('uploading', '2026-10-30');
    uploading.assets[0].state = 'new';
    return {ok: true, json: async () => [unsafe, empty, uploading]};
  }]
]) {
  test('downloads retain usable pinned links on ' + name, async () => {
    const f = fixture(undefined, '', fetchReleases);
    await f.refresh;
    for (const link of f.downloads) assert.ok(link.href.startsWith('pinned/'));
    assert.equal(f.releaseStatus.textContent, 'Pinned preview');
  });
}

test('HTML and README use the same complete preview fallback instead of releases/latest', () => {
  const html = readFileSync(new URL('../site/index.html', import.meta.url), 'utf8');
  const readme = readFileSync(new URL('../README.md', import.meta.url), 'utf8');
  for (const content of [html, readme]) {
    assert.doesNotMatch(content, /releases\/latest\/download/);
    assert.match(content, /releases\/download\/preview-887ded7d5f0b\//);
  }
  assert.equal([...html.matchAll(/data-release-asset=/g)].length, 6);
  assert.match(html, /id="release-status"/);
  assert.doesNotMatch(html, /7443|API_PASSWORD|rustapi:latest/);
});

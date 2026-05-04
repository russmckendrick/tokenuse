import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { generateLatestJson, updaterAssets } from './generate-tauri-latest-json.mjs';

function fixtureDir() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'tokenuse-updater-'));
  for (const { asset } of updaterAssets) {
    fs.writeFileSync(path.join(dir, asset), 'asset');
    fs.writeFileSync(path.join(dir, `${asset}.sig`), `signature-${asset}`);
  }
  return dir;
}

test('generates a complete static Tauri updater manifest', () => {
  const dir = fixtureDir();
  const manifest = JSON.parse(
    generateLatestJson({
      releaseDir: dir,
      repo: 'russmckendrick/tokenuse',
      tag: 'v1.2.3',
      pubDate: '2026-05-03T00:00:00.000Z'
    })
  );

  assert.equal(manifest.version, '1.2.3');
  assert.equal(manifest.pub_date, '2026-05-03T00:00:00.000Z');
  assert.deepEqual(Object.keys(manifest.platforms).sort(), [
    'linux-aarch64',
    'linux-x86_64',
    'windows-x86_64'
  ]);
  assert.equal(
    manifest.platforms['windows-x86_64'].url,
    'https://github.com/russmckendrick/tokenuse/releases/download/v1.2.3/tokenuse-desktop-windows-amd64-setup.exe'
  );
  assert.equal(
    manifest.platforms['linux-aarch64'].signature,
    'signature-tokenuse-desktop-linux-arm64.AppImage'
  );
});

test('fails when a required signature is missing', () => {
  const dir = fixtureDir();
  fs.rmSync(path.join(dir, 'tokenuse-desktop-linux-amd64.AppImage.sig'));

  assert.throws(
    () =>
      generateLatestJson({
        releaseDir: dir,
        repo: 'russmckendrick/tokenuse',
        tag: 'v1.2.3'
      }),
    /missing updater signature/
  );
});

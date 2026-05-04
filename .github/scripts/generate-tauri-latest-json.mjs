#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const updaterAssets = [
  {
    platform: 'windows-x86_64',
    asset: 'tokenuse-desktop-windows-amd64-setup.exe'
  },
  {
    platform: 'linux-x86_64',
    asset: 'tokenuse-desktop-linux-amd64.AppImage'
  },
  {
    platform: 'linux-aarch64',
    asset: 'tokenuse-desktop-linux-arm64.AppImage'
  }
];

export function generateLatestJson({ releaseDir, repo, tag, pubDate = new Date().toISOString() }) {
  if (!tag?.startsWith('v')) {
    throw new Error(`release tag must start with v: ${tag}`);
  }
  if (!repo?.includes('/')) {
    throw new Error(`GitHub repository must be owner/name: ${repo}`);
  }

  const version = tag.slice(1);
  const platforms = {};

  for (const { platform, asset } of updaterAssets) {
    const assetPath = path.join(releaseDir, asset);
    const signaturePath = `${assetPath}.sig`;
    if (!fs.existsSync(assetPath)) {
      throw new Error(`missing updater asset: ${asset}`);
    }
    if (!fs.existsSync(signaturePath)) {
      throw new Error(`missing updater signature: ${asset}.sig`);
    }

    const signature = fs.readFileSync(signaturePath, 'utf8').trim();
    if (!signature) {
      throw new Error(`empty updater signature: ${asset}.sig`);
    }

    platforms[platform] = {
      signature,
      url: `https://github.com/${repo}/releases/download/${tag}/${asset}`
    };
  }

  return `${JSON.stringify(
    {
      version,
      pub_date: pubDate,
      notes: 'See the GitHub Release notes for this version.',
      platforms
    },
    null,
    2
  )}\n`;
}

function parseArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith('--') || value === undefined) {
      throw new Error(`invalid arguments near ${key ?? '<end>'}`);
    }
    args[key.slice(2)] = value;
  }
  for (const key of ['release-dir', 'repo', 'tag', 'out']) {
    if (!args[key]) {
      throw new Error(`missing --${key}`);
    }
  }
  return args;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const latestJson = generateLatestJson({
    releaseDir: args['release-dir'],
    repo: args.repo,
    tag: args.tag
  });
  fs.writeFileSync(args.out, latestJson);
}

const currentFile = fileURLToPath(import.meta.url);
if (process.argv[1] === currentFile) {
  main();
}

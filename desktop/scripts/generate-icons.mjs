import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { deflateSync } from 'node:zlib';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const iconsDir = join(root, 'src-tauri', 'icons');
const appIconSource = 'tokenusebars.svg';
const menubarIconSource = 'tokenusebars-menubar.svg';
const trayIconSource = 'tokenusebars-tray.svg';
const tauriCli = join(root, 'node_modules', '@tauri-apps', 'cli', 'tauri.js');

const tauriIcon = spawnSync(
  process.execPath,
  [tauriCli, 'icon', appIconSource, '--output', 'src-tauri/icons'],
  { cwd: root, stdio: 'inherit' }
);

if (tauriIcon.error) {
  console.error(`Failed to run Tauri icon generator: ${tauriIcon.error.message}`);
  process.exit(1);
}

if (tauriIcon.status !== 0) {
  process.exit(tauriIcon.status ?? 1);
}

mkdirSync(iconsDir, { recursive: true });
writeFileSync(join(iconsDir, 'tray-menubar.png'), renderTrayIcon(menubarIconSource));
writeFileSync(join(iconsDir, 'tray-system.png'), renderTrayIcon(trayIconSource));

function renderTrayIcon(sourceFile) {
  const size = 32;
  const scale = 4;
  const icon = readSvgIcon(sourceFile);
  const rgba = new Uint8Array(size * size * 4);

  for (let py = 0; py < size; py += 1) {
    for (let px = 0; px < size; px += 1) {
      let r = 0;
      let g = 0;
      let b = 0;
      let a = 0;

      for (let sy = 0; sy < scale; sy += 1) {
        for (let sx = 0; sx < scale; sx += 1) {
          const sampleX = px + (sx + 0.5) / scale;
          const sampleY = py + (sy + 0.5) / scale;
          const x = icon.viewBox.x + (sampleX / size) * icon.viewBox.width;
          const y = icon.viewBox.y + (sampleY / size) * icon.viewBox.height;
          const rect = icon.rects.find((candidate) =>
            roundedRectContains(x, y, candidate, candidate.rx)
          );
          if (!rect) continue;

          const color = paintColor(rect.fill, y, icon.gradients);
          r += color.r;
          g += color.g;
          b += color.b;
          a += 255;
        }
      }

      const samples = scale * scale;
      const alpha = a / samples;
      const idx = (py * size + px) * 4;
      rgba[idx] = alpha === 0 ? 0 : Math.round(r / samples / (alpha / 255));
      rgba[idx + 1] = alpha === 0 ? 0 : Math.round(g / samples / (alpha / 255));
      rgba[idx + 2] = alpha === 0 ? 0 : Math.round(b / samples / (alpha / 255));
      rgba[idx + 3] = Math.round(alpha);
    }
  }

  return encodePng(size, size, rgba);
}

function readSvgIcon(sourceFile) {
  const svg = readFileSync(join(root, sourceFile), 'utf8');
  const viewBox = parseViewBox(svg, sourceFile);
  const gradients = parseLinearGradients(svg);
  const rects = [...svg.matchAll(/<rect\b([^>]*)\/?>/g)].map((match) => {
    const attrs = parseAttributes(match[1]);
    return {
      x: numberAttribute(attrs, 'x', 0),
      y: numberAttribute(attrs, 'y', 0),
      w: numberAttribute(attrs, 'width'),
      h: numberAttribute(attrs, 'height'),
      rx: numberAttribute(attrs, 'rx', 0),
      fill: attrs.fill
    };
  });

  if (!rects.length || rects.some((rect) => !rect.fill)) {
    throw new Error(`${sourceFile} must contain filled rect bars`);
  }

  return { viewBox, gradients, rects };
}

function parseViewBox(svg, sourceFile) {
  const match = svg.match(/viewBox="([^"]+)"/);
  if (!match) {
    throw new Error(`${sourceFile} must define a viewBox`);
  }

  const [x, y, width, height] = match[1].trim().split(/\s+/).map(Number);
  if (![x, y, width, height].every(Number.isFinite) || width <= 0 || height <= 0) {
    throw new Error(`${sourceFile} has an invalid viewBox`);
  }

  return { x, y, width, height };
}

function parseLinearGradients(svg) {
  const gradients = new Map();
  for (const match of svg.matchAll(/<linearGradient\b([^>]*)>([\s\S]*?)<\/linearGradient>/g)) {
    const attrs = parseAttributes(match[1]);
    const stops = [...match[2].matchAll(/<stop\b([^>]*)\/?>/g)]
      .map((stopMatch) => parseAttributes(stopMatch[1]))
      .map((stop) => ({
        offset: parseOffset(stop.offset),
        color: parseHexColor(stop['stop-color'])
      }))
      .sort((a, b) => a.offset - b.offset);

    gradients.set(attrs.id, {
      y1: numberAttribute(attrs, 'y1', 0),
      y2: numberAttribute(attrs, 'y2', 1),
      stops
    });
  }
  return gradients;
}

function parseAttributes(source) {
  const attrs = {};
  for (const match of source.matchAll(/([:\w-]+)="([^"]*)"/g)) {
    attrs[match[1]] = match[2];
  }
  return attrs;
}

function numberAttribute(attrs, name, fallback = undefined) {
  if (attrs[name] === undefined) {
    if (fallback !== undefined) return fallback;
    throw new Error(`Missing numeric SVG attribute: ${name}`);
  }

  const value = Number(attrs[name]);
  if (!Number.isFinite(value)) {
    throw new Error(`Invalid numeric SVG attribute ${name}: ${attrs[name]}`);
  }
  return value;
}

function roundedRectContains(x, y, rect, radius) {
  if (x < rect.x || x > rect.x + rect.w || y < rect.y || y > rect.y + rect.h) {
    return false;
  }

  const cx = Math.max(rect.x + radius, Math.min(x, rect.x + rect.w - radius));
  const cy = Math.max(rect.y + radius, Math.min(y, rect.y + rect.h - radius));
  return (x - cx) ** 2 + (y - cy) ** 2 <= radius ** 2;
}

function paintColor(fill, y, gradients) {
  if (fill.startsWith('#')) {
    return parseHexColor(fill);
  }

  const gradientMatch = fill.match(/^url\(#([^)]+)\)$/);
  if (!gradientMatch) {
    throw new Error(`Unsupported SVG fill: ${fill}`);
  }

  const gradient = gradients.get(gradientMatch[1]);
  if (!gradient || gradient.stops.length === 0) {
    throw new Error(`Missing SVG gradient: ${gradientMatch[1]}`);
  }

  return gradientColor(gradient, y);
}

function gradientColor(gradient, y) {
  const span = gradient.y2 - gradient.y1 || 1;
  const t = Math.max(0, Math.min(1, (y - gradient.y1) / span));
  let lower = gradient.stops[0];
  let upper = gradient.stops[gradient.stops.length - 1];

  for (let i = 1; i < gradient.stops.length; i += 1) {
    if (t <= gradient.stops[i].offset) {
      lower = gradient.stops[i - 1];
      upper = gradient.stops[i];
      break;
    }
  }

  const stopSpan = upper.offset - lower.offset || 1;
  const local = Math.max(0, Math.min(1, (t - lower.offset) / stopSpan));
  return {
    r: Math.round(lerp(lower.color.r, upper.color.r, local)),
    g: Math.round(lerp(lower.color.g, upper.color.g, local)),
    b: Math.round(lerp(lower.color.b, upper.color.b, local))
  };
}

function parseOffset(value) {
  if (!value) return 0;
  return value.endsWith('%') ? Number(value.slice(0, -1)) / 100 : Number(value);
}

function parseHexColor(value) {
  const match = value?.match(/^#([\da-f]{6})$/i);
  if (!match) {
    throw new Error(`Unsupported SVG color: ${value}`);
  }

  const hex = Number.parseInt(match[1], 16);
  return {
    r: (hex >> 16) & 0xff,
    g: (hex >> 8) & 0xff,
    b: hex & 0xff
  };
}

function lerp(a, b, t) {
  return a + (b - a) * t;
}

function encodePng(width, height, rgba) {
  const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;

  const raw = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y += 1) {
    const row = y * (width * 4 + 1);
    raw[row] = 0;
    Buffer.from(rgba.buffer, y * width * 4, width * 4).copy(raw, row + 1);
  }

  return Buffer.concat([
    signature,
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw)),
    chunk('IEND', Buffer.alloc(0))
  ]);
}

function chunk(type, data) {
  const typeBuffer = Buffer.from(type);
  const out = Buffer.alloc(12 + data.length);
  out.writeUInt32BE(data.length, 0);
  typeBuffer.copy(out, 4);
  data.copy(out, 8);
  out.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])), out.length - 4);
  return out;
}

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc ^= byte;
    for (let i = 0; i < 8; i += 1) {
      crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

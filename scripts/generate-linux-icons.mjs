import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { deflateSync } from 'node:zlib';

const outputDir = join(process.cwd(), 'build', 'icons');
const sizes = [16, 32, 48, 64, 128, 256, 512, 1024];

await mkdir(outputDir, { recursive: true });

for (const size of sizes) {
  const png = renderIcon(size);
  await writeFile(join(outputDir, `${size}x${size}.png`), png);
}

console.log(`Generated ${sizes.length} Linux icons in ${outputDir}`);

function renderIcon(size) {
  const width = size;
  const height = size;
  const pixels = Buffer.alloc(width * height * 4);

  const margin = Math.max(1, Math.round(size * 0.055));
  const radius = Math.round(size * 0.18);
  const docX = Math.round(size * 0.22);
  const docY = Math.round(size * 0.16);
  const docW = Math.round(size * 0.56);
  const docH = Math.round(size * 0.68);
  const docR = Math.max(1, Math.round(size * 0.045));

  roundedRect(pixels, width, height, margin, margin, size - margin * 2, size - margin * 2, radius, [15, 118, 110, 255]);
  roundedRect(pixels, width, height, margin, margin, size - margin * 2, Math.round(size * 0.44), radius, [20, 184, 166, 255]);

  roundedRect(pixels, width, height, docX, docY, docW, docH, docR, [248, 250, 252, 255]);
  rect(pixels, width, height, docX + docW - Math.round(size * 0.16), docY, Math.round(size * 0.16), Math.round(size * 0.16), [226, 232, 240, 255]);
  triangle(pixels, width, height, docX + docW - Math.round(size * 0.16), docY, docX + docW, docY, docX + docW, docY + Math.round(size * 0.16), [203, 213, 225, 255]);

  const line = Math.max(1, Math.round(size * 0.018));
  const gridColor = [20, 184, 166, 255];
  const top = docY + Math.round(size * 0.26);
  const left = docX + Math.round(size * 0.09);
  const right = docX + docW - Math.round(size * 0.09);
  const bottom = docY + docH - Math.round(size * 0.18);
  const rowGap = Math.max(2, Math.round((bottom - top) / 3));
  const colGap = Math.max(2, Math.round((right - left) / 3));

  for (let i = 0; i <= 3; i++) {
    rect(pixels, width, height, left, top + i * rowGap, right - left, line, gridColor);
    rect(pixels, width, height, left + i * colGap, top, line, bottom - top, gridColor);
  }

  const dotR = Math.max(1, Math.round(size * 0.018));
  for (const [cx, cy] of [
    [left + colGap / 2, top + rowGap / 2],
    [left + colGap * 1.5, top + rowGap * 1.5],
    [left + colGap * 2.5, top + rowGap * 2.5],
  ]) {
    circle(pixels, width, height, Math.round(cx), Math.round(cy), dotR, [15, 118, 110, 255]);
  }

  const shieldCx = Math.round(size * 0.66);
  const shieldCy = Math.round(size * 0.68);
  shield(pixels, width, height, shieldCx, shieldCy, Math.round(size * 0.14), [250, 204, 21, 255], [15, 23, 42, 255]);

  return encodePng(width, height, pixels);
}

function roundedRect(pixels, width, height, x, y, w, h, r, color) {
  for (let py = y; py < y + h; py++) {
    for (let px = x; px < x + w; px++) {
      const dx = px < x + r ? x + r - px : px >= x + w - r ? px - (x + w - r - 1) : 0;
      const dy = py < y + r ? y + r - py : py >= y + h - r ? py - (y + h - r - 1) : 0;
      if (dx * dx + dy * dy <= r * r) {
        setPixel(pixels, width, height, px, py, color);
      }
    }
  }
}

function rect(pixels, width, height, x, y, w, h, color) {
  for (let py = Math.max(0, y); py < Math.min(height, y + h); py++) {
    for (let px = Math.max(0, x); px < Math.min(width, x + w); px++) {
      setPixel(pixels, width, height, px, py, color);
    }
  }
}

function triangle(pixels, width, height, x1, y1, x2, y2, x3, y3, color) {
  const minX = Math.floor(Math.min(x1, x2, x3));
  const maxX = Math.ceil(Math.max(x1, x2, x3));
  const minY = Math.floor(Math.min(y1, y2, y3));
  const maxY = Math.ceil(Math.max(y1, y2, y3));
  const area = edge(x1, y1, x2, y2, x3, y3);

  for (let y = minY; y <= maxY; y++) {
    for (let x = minX; x <= maxX; x++) {
      const w1 = edge(x2, y2, x3, y3, x, y);
      const w2 = edge(x3, y3, x1, y1, x, y);
      const w3 = edge(x1, y1, x2, y2, x, y);
      if ((area >= 0 && w1 >= 0 && w2 >= 0 && w3 >= 0) || (area < 0 && w1 <= 0 && w2 <= 0 && w3 <= 0)) {
        setPixel(pixels, width, height, x, y, color);
      }
    }
  }
}

function circle(pixels, width, height, cx, cy, r, color) {
  for (let y = cy - r; y <= cy + r; y++) {
    for (let x = cx - r; x <= cx + r; x++) {
      const dx = x - cx;
      const dy = y - cy;
      if (dx * dx + dy * dy <= r * r) {
        setPixel(pixels, width, height, x, y, color);
      }
    }
  }
}

function shield(pixels, width, height, cx, cy, r, fill, stroke) {
  const points = [
    [cx, cy - r],
    [cx + r, cy - Math.round(r * 0.45)],
    [cx + Math.round(r * 0.72), cy + Math.round(r * 0.75)],
    [cx, cy + r],
    [cx - Math.round(r * 0.72), cy + Math.round(r * 0.75)],
    [cx - r, cy - Math.round(r * 0.45)],
  ];

  polygon(pixels, width, height, points, stroke);
  const inner = points.map(([x, y]) => [Math.round(cx + (x - cx) * 0.78), Math.round(cy + (y - cy) * 0.78)]);
  polygon(pixels, width, height, inner, fill);

  const check = Math.max(1, Math.round(r * 0.16));
  rect(pixels, width, height, cx - Math.round(r * 0.42), cy + Math.round(r * 0.05), Math.round(r * 0.35), check, stroke);
  rect(pixels, width, height, cx - Math.round(r * 0.1), cy - Math.round(r * 0.25), check, Math.round(r * 0.6), stroke);
}

function polygon(pixels, width, height, points, color) {
  const xs = points.map(([x]) => x);
  const ys = points.map(([, y]) => y);
  const minX = Math.floor(Math.min(...xs));
  const maxX = Math.ceil(Math.max(...xs));
  const minY = Math.floor(Math.min(...ys));
  const maxY = Math.ceil(Math.max(...ys));

  for (let y = minY; y <= maxY; y++) {
    for (let x = minX; x <= maxX; x++) {
      if (pointInPolygon(x, y, points)) {
        setPixel(pixels, width, height, x, y, color);
      }
    }
  }
}

function pointInPolygon(x, y, points) {
  let inside = false;
  for (let i = 0, j = points.length - 1; i < points.length; j = i++) {
    const [xi, yi] = points[i];
    const [xj, yj] = points[j];
    const intersect = yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi;
    if (intersect) inside = !inside;
  }
  return inside;
}

function setPixel(pixels, width, height, x, y, color) {
  if (x < 0 || y < 0 || x >= width || y >= height) return;
  const offset = (y * width + x) * 4;
  pixels[offset] = color[0];
  pixels[offset + 1] = color[1];
  pixels[offset + 2] = color[2];
  pixels[offset + 3] = color[3];
}

function edge(x1, y1, x2, y2, x, y) {
  return (x - x1) * (y2 - y1) - (y - y1) * (x2 - x1);
}

function encodePng(width, height, rgba) {
  const raw = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y++) {
    const rawOffset = y * (width * 4 + 1);
    raw[rawOffset] = 0;
    rgba.copy(raw, rawOffset + 1, y * width * 4, (y + 1) * width * 4);
  }

  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk('IHDR', ihdr(width, height)),
    chunk('IDAT', deflateSync(raw)),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

function ihdr(width, height) {
  const buffer = Buffer.alloc(13);
  buffer.writeUInt32BE(width, 0);
  buffer.writeUInt32BE(height, 4);
  buffer[8] = 8;
  buffer[9] = 6;
  buffer[10] = 0;
  buffer[11] = 0;
  buffer[12] = 0;
  return buffer;
}

function chunk(type, data) {
  const typeBuffer = Buffer.from(type);
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])), 0);
  return Buffer.concat([length, typeBuffer, data, crc]);
}

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) {
      crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

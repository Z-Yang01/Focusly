// 生成 Focusly 应用图标源图 (assets/icon.png, 1024x1024 RGBA)
// 纯 Node 实现 PNG 编码，无第三方依赖。随后由 `tauri icon` 生成全平台尺寸。
import { deflateSync } from "node:zlib";
import { writeFileSync, mkdirSync } from "node:fs";
import path from "node:path";

const SIZE = 1024;

// ---------- PNG 编码 ----------
const crcTable = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = crcTable[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const typeBuf = Buffer.from(type, "ascii");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])));
  return Buffer.concat([len, typeBuf, data, crc]);
}

function encodePng(width, height, rgba) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // color type RGBA
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;
  // 每行前加 filter byte 0
  const raw = Buffer.alloc(height * (width * 4 + 1));
  for (let y = 0; y < height; y++) {
    raw[y * (width * 4 + 1)] = 0;
    rgba.copy(raw, y * (width * 4 + 1) + 1, y * width * 4, (y + 1) * width * 4);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

// ---------- 绘制 ----------
const px = new Uint8ClampedArray(SIZE * SIZE * 4);

function setPixel(x, y, r, g, b, a) {
  const i = (y * SIZE + x) * 4;
  const sa = a / 255;
  const da = px[i + 3] / 255;
  const oa = sa + da * (1 - sa);
  if (oa <= 0) return;
  px[i] = (r * sa + px[i] * da * (1 - sa)) / oa;
  px[i + 1] = (g * sa + px[i + 1] * da * (1 - sa)) / oa;
  px[i + 2] = (b * sa + px[i + 2] * da * (1 - sa)) / oa;
  px[i + 3] = oa * 255;
}

const sdRoundedRect = (x, y, cx, cy, hw, hh, r) => {
  const dx = Math.abs(x - cx) - (hw - r);
  const dy = Math.abs(y - cy) - (hh - r);
  const ox = Math.max(dx, 0);
  const oy = Math.max(dy, 0);
  return Math.min(Math.max(dx, dy), 0) - r + Math.hypot(ox, oy);
};

const smoothAlpha = (d) => Math.max(0, Math.min(1, 0.5 - d));

// 背景：圆角方块 + 对角渐变 (靛蓝 -> 紫罗兰)
const BG_CX = SIZE / 2,
  BG_CY = SIZE / 2,
  BG_HW = SIZE / 2 - 96,
  BG_HH = BG_HW,
  BG_R = 210;
const c1 = [99, 102, 241]; // indigo-500
const c2 = [139, 92, 246]; // violet-500

// 前景：三行"便签横线" + 一个完成勾
const bars = [
  { y: 430, hw: 250, hh: 34 },
  { y: 542, hw: 250, hh: 34 },
  { y: 654, hw: 155, hh: 34 },
];

// 勾的两条线段 (位于右上角圆形徽标内)
const badge = { cx: 768, cy: 300, r: 118 };
const checkSegs = [
  // 短边
  { x1: 706, y1: 300, x2: 752, y2: 350, hw: 17 },
  // 长边
  { x1: 752, y1: 350, x2: 842, y2: 252, hw: 17 },
];

const sdSegment = (x, y, seg) => {
  const vx = seg.x2 - seg.x1,
    vy = seg.y2 - seg.y1;
  const wx = x - seg.x1,
    wy = y - seg.y1;
  const t = Math.max(0, Math.min(1, (wx * vx + wy * vy) / (vx * vx + vy * vy)));
  const dx = x - (seg.x1 + vx * t),
    dy = y - (seg.y1 + vy * t);
  return Math.hypot(dx, dy) - seg.hw;
};

for (let y = 0; y < SIZE; y++) {
  for (let x = 0; x < SIZE; x++) {
    const d = sdRoundedRect(x, y, BG_CX, BG_CY, BG_HW, BG_HH, BG_R);
    const a = smoothAlpha(d) * 255;
    if (a <= 0) continue;
    const t = (x + y) / (2 * SIZE);
    const r = c1[0] + (c2[0] - c1[0]) * t;
    const g = c1[1] + (c2[1] - c1[1]) * t;
    const b = c1[2] + (c2[2] - c1[2]) * t;
    setPixel(x, y, r, g, b, a);
  }
}

// 白色横线（带轻微透明）
for (const bar of bars) {
  for (let y = Math.floor(bar.y - bar.hh - 2); y <= Math.ceil(bar.y + bar.hh + 2); y++) {
    for (let x = 232; x <= 232 + bar.hw * 2 + 4; x++) {
      const d = sdRoundedRect(x, y, 232 + bar.hw, bar.y, bar.hw, bar.hh, bar.hh);
      const a = smoothAlpha(d) * 255;
      if (a > 0) setPixel(x, y, 255, 255, 255, a);
    }
  }
}

// 完成徽标：白色圆底 + 渐变勾
for (let y = badge.cy - badge.r - 4; y <= badge.cy + badge.r + 4; y++) {
  for (let x = badge.cx - badge.r - 4; x <= badge.cx + badge.r + 4; x++) {
    const d = Math.hypot(x - badge.cx, y - badge.cy) - badge.r;
    const a = smoothAlpha(d) * 255;
    if (a > 0) setPixel(x, y, 255, 255, 255, a);
  }
}
for (const seg of checkSegs) {
  for (let y = seg.y1 - 60; y <= seg.y2 + 60; y++) {
    for (let x = seg.x1 - 60; x <= seg.x2 + 60; x++) {
      if (x < 0 || y < 0 || x >= SIZE || y >= SIZE) continue;
      if (Math.hypot(x - badge.cx, y - badge.cy) > badge.r - 18) continue;
      const d = sdSegment(x, y, seg);
      const a = smoothAlpha(d) * 255;
      if (a > 0) setPixel(x, y, 109, 100, 240, a);
    }
  }
}

const out = encodePng(SIZE, SIZE, Buffer.from(px.buffer));
mkdirSync(path.resolve("assets"), { recursive: true });
writeFileSync(path.resolve("assets/icon.png"), out);
console.log(`icon written: assets/icon.png (${SIZE}x${SIZE}, ${out.length} bytes)`);

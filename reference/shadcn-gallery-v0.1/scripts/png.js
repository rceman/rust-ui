// Minimal PNG decoder - 8-bit RGB(A)/palette/gray, all filters, no interlace.
// Just enough to read Chromium's deterministic screenshots for the
// text-ink validation. Decodes to {width, height, data: Uint8Array RGBA}.
const zlib = require("node:zlib");

function decodePng(buf) {
  if (buf.readUInt32BE(0) !== 0x89504e47) throw new Error("not a PNG");
  let off = 8;
  let w = 0, h = 0, bitDepth = 0, colorType = 0, interlace = 0;
  let idat = [];
  let plte = null, trns = null;
  while (off < buf.length) {
    const len = buf.readUInt32BE(off);
    const type = buf.toString("ascii", off + 4, off + 8);
    const data = buf.subarray(off + 8, off + 8 + len);
    if (type === "IHDR") {
      w = data.readUInt32BE(0); h = data.readUInt32BE(4);
      bitDepth = data[8]; colorType = data[9]; interlace = data[12];
    } else if (type === "IDAT") idat.push(data);
    else if (type === "PLTE") plte = data;
    else if (type === "tRNS") trns = data;
    else if (type === "IEND") break;
    off += 12 + len;
  }
  if (interlace) throw new Error("interlaced PNG unsupported");
  if (bitDepth !== 8) throw new Error(`bit depth ${bitDepth} unsupported`);
  const ch = { 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 }[colorType];
  if (!ch) throw new Error(`color type ${colorType} unsupported`);
  const raw = zlib.inflateSync(Buffer.concat(idat));
  const stride = w * ch;
  const out = Buffer.alloc(h * stride);
  let pos = 0;
  for (let y = 0; y < h; y++) {
    const f = raw[pos++];
    const row = out.subarray(y * stride, (y + 1) * stride);
    const prev = y ? out.subarray((y - 1) * stride, y * stride) : null;
    raw.copy(row, 0, pos, pos + stride);
    pos += stride;
    for (let x = 0; x < stride; x++) {
      const a = x >= ch ? row[x - ch] : 0;
      const b = prev ? prev[x] : 0;
      const c = x >= ch && prev ? prev[x - ch] : 0;
      if (f === 1) row[x] = (row[x] + a) & 255;
      else if (f === 2) row[x] = (row[x] + b) & 255;
      else if (f === 3) row[x] = (row[x] + ((a + b) >> 1)) & 255;
      else if (f === 4) {
        const p = a + b - c;
        const pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
        row[x] = (row[x] + (pa <= pb && pa <= pc ? a : pb <= pc ? b : c)) & 255;
      }
    }
  }
  // expand to RGBA
  const rgba = new Uint8Array(w * h * 4);
  for (let i = 0; i < w * h; i++) {
    if (colorType === 6) {
      rgba[i * 4] = out[i * 4]; rgba[i * 4 + 1] = out[i * 4 + 1];
      rgba[i * 4 + 2] = out[i * 4 + 2]; rgba[i * 4 + 3] = out[i * 4 + 3];
    } else if (colorType === 2) {
      rgba[i * 4] = out[i * 3]; rgba[i * 4 + 1] = out[i * 3 + 1];
      rgba[i * 4 + 2] = out[i * 3 + 2]; rgba[i * 4 + 3] = 255;
    } else if (colorType === 0) {
      const g = out[i];
      rgba[i * 4] = rgba[i * 4 + 1] = rgba[i * 4 + 2] = g; rgba[i * 4 + 3] = 255;
    } else if (colorType === 3) {
      const idx = out[i];
      rgba[i * 4] = plte[idx * 3]; rgba[i * 4 + 1] = plte[idx * 3 + 1];
      rgba[i * 4 + 2] = plte[idx * 3 + 2];
      rgba[i * 4 + 3] = trns ? trns[idx] : 255;
    } else if (colorType === 4) {
      const g = out[i * 2], a = out[i * 2 + 1];
      rgba[i * 4] = rgba[i * 4 + 1] = rgba[i * 4 + 2] = g; rgba[i * 4 + 3] = a;
    }
  }
  return { width: w, height: h, data: rgba };
}

module.exports = { decodePng };

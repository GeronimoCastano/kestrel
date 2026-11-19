'use strict';

const fs = require('fs');
const path = require('path');
const os = require('os');
const { createHash } = require('crypto');
const { pathToFileURL } = require('url');
let directory = path.join(os.tmpdir(), 'kestrel-doc-math-v2');
const cache = new Map();
const LIMIT = 128;

function configure(storage) {
  directory = storage ? path.join(storage, 'doc-math-v2') : path.join(os.tmpdir(), 'kestrel-doc-math-v2');
  cache.clear();
  try {
    const old = fs.readdirSync(directory).filter(name => /^[a-f0-9]{64}\.svg$/.test(name))
      .map(name => ({ name, modified: fs.statSync(path.join(directory, name)).mtimeMs }))
      .sort((a, b) => b.modified - a.modified);
    for (const file of old.slice(LIMIT)) fs.unlinkSync(path.join(directory, file.name));
  } catch (_) { /* the cache need not exist until the first equation */ }
}

function image(expression, display, color, fontSize) {
  if (expression.length > 4000) return null;
  const key = createHash('sha256').update(`${color}:${fontSize}:${display}:${expression}`).digest('hex');
  const file = path.join(directory, `${key}.svg`);
  try {
    if (!cache.has(key) || !fs.existsSync(file)) {
      let svg = require('./math-svg')(expression, display);
      if (svg.length > 200000) return null;
      const box = /viewBox="([^"]+)"/.exec(svg)?.[1].split(/\s+/).map(Number);
      if (!box || box.length !== 4 || box.some(value => !Number.isFinite(value))) return null;
      let [x, y, width, height] = box;
      if (!display) {
        // MathJax's TeX glyphs use 1000 units/em, with the math axis at 250.
        // Balance the viewport about that axis. Native middle alignment then
        // aligns every formula alike, including deep fractions and subscripts.
        const radius = Math.max(-250 - y, y + height + 250);
        y = -250 - radius;
        height = radius * 2;
      }
      svg = svg.replace(/currentColor/g, color)
        .replace(/viewBox="[^"]+"/, `viewBox="${x} ${y} ${width} ${height}"`)
        .replace(/width="[^"]+"/, `width="${width * fontSize / 1000}"`)
        .replace(/height="[^"]+"/, `height="${height * fontSize / 1000}"`);
      fs.mkdirSync(directory, { recursive: true });
      const temporary = `${file}.${process.pid}.tmp`;
      fs.writeFileSync(temporary, svg);
      fs.renameSync(temporary, file);
    }
    cache.delete(key);
    cache.set(key, file);
    while (cache.size > LIMIT) {
      const oldest = cache.keys().next().value;
      try { fs.unlinkSync(cache.get(oldest)); } catch (_) { /* another host may have cleared it */ }
      cache.delete(oldest);
    }
    if (!display) {
      const alt = expression.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/\s+/g, ' ');
      // HTML center maps to CSS middle; HTML middle instead centers on the baseline.
      return `<img src="${pathToFileURL(file).href}" alt="${alt}" align="center">`;
    }
    const alt = expression.replace(/[\\\[\]<>]/g, '\\$&').replace(/\s+/g, ' ');
    return `![${alt}](${pathToFileURL(file).href})`;
  } catch (_) {
    // Incomplete/unsupported TeX stays readable while the user is editing it.
    return null;
  }
}

// Preserve Markdown code, escaped dollars and link destinations.
function render(text, color = '#333333', size = 14) {
  if (!text || !text.includes('$')) return text || '';
  const fontSize = Number.isFinite(size) ? Math.max(6, Math.min(100, size)) : 14;
  let result = '', index = 0, fence = '';
  while (index < text.length) {
    if (index === 0 || text[index - 1] === '\n') {
      const line = text.slice(index).match(/^[ \t]{0,3}(`{3,}|~{3,})[^\n]*(?:\n|$)/);
      if (line && (!fence || line[1][0] === fence[0] && line[1].length >= fence.length)) {
        fence = fence ? '' : line[1];
        result += line[0]; index += line[0].length; continue;
      }
    }
    if (fence) { result += text[index++]; continue; }
    if (text[index] === '\\' && index + 1 < text.length) {
      result += text.slice(index, index + 2); index += 2; continue;
    }
    if (text[index] === '`') {
      const ticks = /^`+/.exec(text.slice(index))[0];
      const end = text.indexOf(ticks, index + ticks.length);
      if (end >= 0) { result += text.slice(index, end + ticks.length); index = end + ticks.length; continue; }
    }
    const protectedPart = /^(?:!?\[[^\]\n]*\]\([^\n]*?\)|<(?:[a-z][\w+.-]*:[^>\s]+|[^>\s]+@[^>\s]+)>)/i.exec(text.slice(index));
    if (protectedPart) { result += protectedPart[0]; index += protectedPart[0].length; continue; }
    // The popup enables HTML only for our generated, baseline-aligned images.
    // Keep authored HTML inert as it was before math image support.
    const html = /^(?:<!--[\s\S]*?-->|<\/?[a-z][a-z0-9-]*(?:\s[^>]*|\/?)>)/i.exec(text.slice(index));
    if (html) { index += html[0].length; continue; }
    if (text[index] !== '$') { result += text[index++]; continue; }
    const display = text[index + 1] === '$';
    const marker = display ? '$$' : '$';
    const from = index + marker.length;
    if (!display && /\s|\$/.test(text[from] || ' ')) { result += text[index++]; continue; }
    let end = from;
    for (; end < text.length; end += 1) {
      if (text[end] === '\\') { end += 1; continue; }
      if (!display && text.slice(end, end + 2) === '\n\n') break;
      if (text.startsWith(marker, end) && (display || !/\s/.test(text[end - 1]) && !/\d|\$/.test(text[end + 1] || ''))) break;
    }
    if (!text.startsWith(marker, end) || end === from) { result += text[index++]; continue; }
    const rendered = image(text.slice(from, end), display, color, fontSize);
    const original = text.slice(index, end + marker.length);
    result += rendered ? display ? `\n\n${rendered}\n\n` : rendered : original;
    index = end + marker.length;
  }
  return result;
}

function atComment(text, offset) {
  const start = text.lastIndexOf('\n', offset - 1) + 1;
  const end = text.indexOf('\n', offset);
  if (!/^\s*\/\/\/(?!\/)/.test(text.slice(start, end < 0 ? text.length : end))) return '';
  const lines = text.split('\n');
  let line = text.slice(0, start).split('\n').length - 1;
  let first = line, last = line;
  while (first > 0 && /^\s*\/\/\/(?!\/)/.test(lines[first - 1])) first -= 1;
  while (last + 1 < lines.length && /^\s*\/\/\/(?!\/)/.test(lines[last + 1])) last += 1;
  return lines.slice(first, last + 1).map(value => value.replace(/^\s*\/\/\/ ?/, '').replace(/\r$/, '')).join('\n');
}
module.exports = { render, configure, atComment };

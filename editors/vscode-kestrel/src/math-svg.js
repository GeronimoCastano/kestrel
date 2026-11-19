'use strict';
// Build-time entry: the shipped bundle includes the TeX parser and SVG glyphs.
const { mathjax } = require('@mathjax/src/js/mathjax.js');
const { TeX } = require('@mathjax/src/js/input/tex.js');
const { SVG } = require('@mathjax/src/js/output/svg.js');
const { liteAdaptor } = require('@mathjax/src/js/adaptors/liteAdaptor.js');
const { RegisterHTMLHandler } = require('@mathjax/src/js/handlers/html.js');
const { MathJaxTexFont } = require('@mathjax/mathjax-tex-font/js/svg.js');
require('@mathjax/src/js/input/tex/base/BaseConfiguration.js');
require('@mathjax/src/js/input/tex/ams/AmsConfiguration.js');
require('@mathjax/src/js/input/tex/newcommand/NewcommandConfiguration.js');
const adaptor = liteAdaptor();
RegisterHTMLHandler(adaptor);
// A file image must contain the whole equation, rather than inline SVG fragments.
const output = new SVG({ fontData: MathJaxTexFont, fontCache: 'none', linebreaks: { inline: false } });
module.exports = function svg(expression, display) {
  // A fresh input parser prevents one comment's macros leaking into another.
  const input = new TeX({ packages: ['base', 'ams', 'newcommand'], maxBuffer: 8000, maxMacros: 500,
    formatError(_jax, error) { throw error; } });
  const document = mathjax.document('', { InputJax: input, OutputJax: output });
  const node = document.convert(expression, { display, em: 14, ex: 7, containerWidth: 560 });
  return adaptor.outerHTML(adaptor.firstChild(node));
};

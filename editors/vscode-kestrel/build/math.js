'use strict';
const esbuild = require('esbuild');
const fs = require('fs');
const path = require('path');
esbuild.buildSync({ entryPoints: [path.join(__dirname, '../src/math-svg.js')], bundle: true,
  platform: 'node', target: 'node16', minify: true, legalComments: 'external',
  outfile: path.join(__dirname, '../out/math-svg.js') });
fs.copyFileSync(require.resolve('@mathjax/src/LICENSE'), path.join(__dirname, '../out/MathJax-LICENSE.txt'));

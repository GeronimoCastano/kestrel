'use strict';
const vscode = require('vscode');
const fs = require('fs');
const path = require('path');
const assert = require('assert');
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
exports.run = async function () {
  const root = process.env.KESTREL_VSCODE_QA;
  try {
    const extension = vscode.extensions.getExtension('kestrel-lang.kestrel-language');
    await extension.activate();
    const text = String.raw`/// Computes $\frac{x^2}{2}$ and preserves **Markdown**.
///
/// Relations: $h(s) = \infty$. A heuristic $h : S \to \mathbb{N}_0 \cup \{\infty\}$.
/// Baselines: $h$, $h(s)$, $x^2$, $x_i$, $\frac{p}{q}$.
///
/// $$\sum_{i=0}^{n} i = \frac{n(n+1)}{2}$$
function energy(x : Int) : Int = x
trait Measure {public function width():Int=3}
class Sample {has Measure;new(){}}
let sample := Sample()
let result := sample.width()
print(energy(result))
`;
    const file = path.join(root,'sample.kes');fs.writeFileSync(file,text);
    const doc = await vscode.workspace.openTextDocument(file);
    const editor = await vscode.window.showTextDocument(doc);
    const pos = marker => doc.positionAt(text.lastIndexOf(marker));
    assert.strictEqual(doc.languageId,'kestrel');
    const completions = await vscode.commands.executeCommand('vscode.executeCompletionItemProvider',doc.uri,doc.positionAt(text.indexOf('sample.width')+7));
    assert(completions.items.some(item=>item.label==='width'),'trait default suggestion');
    const result = await vscode.commands.executeCommand('vscode.executeHoverProvider',doc.uri,pos('result'));
    assert(result.some(hover=>hover.contents.some(content=>content.value?.includes('Int'))),'inferred result hover');
    const hovers = await vscode.commands.executeCommand('vscode.executeHoverProvider',doc.uri,pos('energy'));
    const markdown = hovers.flatMap(hover=>hover.contents).map(content=>content.value||'').join('\n');
    assert(markdown.includes('.svg)'),'math image in real hover');
    assert(markdown.includes('**Markdown**'),'ordinary Markdown retained');
    const signatures = await vscode.commands.executeCommand('vscode.executeSignatureHelpProvider',doc.uri,doc.positionAt(text.lastIndexOf('energy(')+7),'(');
    assert(signatures.signatures.some(signature=>signature.documentation?.value?.includes('.svg)')),'math signature docs');
    const comment = await vscode.commands.executeCommand('vscode.executeHoverProvider',doc.uri,doc.positionAt(text.indexOf('frac')));
    assert(comment.some(hover=>hover.contents.some(content=>content.value?.includes('.svg)'))),'math directly over ///');
    const profiles = [
      { phase: 'light', theme: 'Default Light Modern', font: 14, language: 0, zoom: 0 },
      { phase: 'dark', theme: 'Default Dark Modern', font: 14, language: 0, zoom: 0 },
      { phase: 'large', theme: 'Default Dark Modern', font: 24, language: 0, zoom: 0 },
      { phase: 'override', theme: 'Default Light Modern', font: 12, language: 20, zoom: 0 },
      { phase: 'small', theme: 'Default Light Modern', font: 10, language: 0, zoom: 0 },
      { phase: 'zoom', theme: 'Default Dark Modern', font: 18, language: 18, zoom: 1 },
      { phase: 'hc-dark', theme: 'Default High Contrast', font: 16, language: 0, zoom: 0 },
      { phase: 'hc-light', theme: 'Default High Contrast Light', font: 16, language: 0, zoom: 0 }
    ];
    for (const { phase, theme, font, language, zoom } of profiles) {
      await vscode.workspace.getConfiguration('workbench').update('colorTheme',theme,vscode.ConfigurationTarget.Global);
      await vscode.workspace.getConfiguration('editor').update('fontSize',font,vscode.ConfigurationTarget.Global);
      await vscode.workspace.getConfiguration().update('[kestrel]',language ? { 'editor.fontSize': language } : undefined,vscode.ConfigurationTarget.Global);
      await vscode.workspace.getConfiguration('window').update('zoomLevel',zoom,vscode.ConfigurationTarget.Global);
      await pause(800);
      const point = pos('energy');editor.selection=new vscode.Selection(point,point);
      await vscode.commands.executeCommand('editor.action.showHover');
      await pause(500);
      fs.writeFileSync(path.join(root,`${phase}.ready`),JSON.stringify({fontSize:language || font}));
      const deadline=Date.now()+30000;
      while(!fs.existsSync(path.join(root,`${phase}.done`)) && Date.now()<deadline) await pause(100);
      assert(fs.existsSync(path.join(root,`${phase}.done`)),`${phase} image verification`);
      await vscode.commands.executeCommand('editor.action.hideHover');
    }
    fs.writeFileSync(path.join(root,'api-result.json'),JSON.stringify({passed:true,checks:9}));
  } catch(error) {
    fs.writeFileSync(path.join(root,'api-result.json'),JSON.stringify({passed:false,error:error.stack}));throw error;
  }
};

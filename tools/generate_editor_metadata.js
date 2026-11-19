'use strict';
// Regenerate editor/Jupyter declarations from the standard library's Kestrel sources.
const fs = require('fs');
const path = require('path');
const root = path.resolve(__dirname, '..');
const reader = require(path.join(root, 'editors/vscode-kestrel/out/kestrel'));
const entries = [], moduleMap = new Map();
const moduleSource = fs.readFileSync(path.join(root, 'src/modules.rs'), 'utf8');
const sources = [...moduleSource.matchAll(/"(std\.[\w.]+)"\s*,\s*"kestrel_std_[^"]+"\s*,\s*include_str!\("([^"]+)"\)/g)].map(m => [m[1], m[2]]);
sources.push(['std.core', 'prelude.kes'], ['std.core', 'methods.kes']);
function add(entry) {
  entry.parameters ||= [];
  entry.documentation ||= '';
  entries.push(entry);
  if (entry.module && !entry.receiver && !entry.static) {
    const names = moduleMap.get(entry.module) || new Set(); names.add(entry.name); moduleMap.set(entry.module, names);
  }
}
function scopeEnd(text, start) {
  let depth = 1, at = start + 1;
  for (; at < text.length && depth; at++) { if (text[at] === '{') depth++; if (text[at] === '}') depth--; }
  return at;
}
for (const [module, file] of sources) {
  const text = fs.readFileSync(path.join(root, 'src', file), 'utf8');
  const clean = reader.sanitize(text);
  const extensions = [...clean.matchAll(/\bextend\s+(\w+)\s*\{/g)].map(m => ({ owner: m[1], start: m.index, end: scopeEnd(clean, m.index + m[0].length - 1) }));
  // A record's header ends at its closing parenthesis; methods may follow in braces.
  const records = [];
  for (const m of clean.matchAll(/\brecord\s+([A-Z]\w*)[^\n{]*\)\s*\{/g)) {
    records.push({ owner: m[1], start: m.index, end: scopeEnd(clean, m.index + m[0].length - 1) });
  }
  const model = reader.parseDocument(text);
  for (const f of model.functions) {
    const prefix = clean.slice(clean.lastIndexOf('\n', f.start) + 1, f.start);
    if (!/\bpublic\b/.test(prefix)) continue;
    const ext = extensions.find(e => e.start < f.start && f.start < e.end);
    const record = records.find(r => r.start < f.start && f.start < r.end);
    const parameters = f.parameters.slice(ext ? 1 : 0).map(p => p.raw || `${p.name} : ${p.type}`);
    const receiver = ext ? f.parameters[0]?.type || ext.owner : record ? record.owner : (!f.static ? f.owner : '');
    const header = text.slice(f.start, f.paramsOpen).replace(/\s+/g, ' ').replace(/^function\s+/, '').trim();
    const entry = { name: f.name, signature: `${receiver || (f.static ? f.owner : module)}.${header}(${parameters.join(', ')}) : ${f.result || 'void'}`, parameters, module };
    if (receiver) entry.receiver = receiver;
    else if (f.static) entry.static = f.owner;
    else entry.gated = module !== 'std.core';
    add(entry);
  }
  for (const m of clean.matchAll(/\bpublic\s+(class|enum|record|trait)\s+([A-Z]\w*)(\[[^\n{]*?\])?/g)) {
    add({ name: m[2], type: true, module, gated: module !== 'std.core', signature: `${m[1]} ${m[2]}${m[3] || ''}` });
  }
}
function native(module, name, parameters, result, effects = false) {
  add({ module, name, gated: true, parameters, signature: `${module}.${name}(${parameters.join(', ')}) : ${result}${effects ? ' with Effects' : ''}` });
}
for (const [name, params, result] of [
 ['readFile',['path : String'],'String or Error'], ['readBytes',['path : String'],'Bytes or Error'],
 ...['writeFile','appendFile'].map(n=>[n,['path : String','text : String'],'void or Error']),
 ...['writeBytes','appendBytes'].map(n=>[n,['path : String','data : Bytes'],'void or Error']),
 ...['exists','isDir'].map(n=>[n,['path : String'],'Bool']),
 ['listDir',['path : String'],'List[String] or Error'], ...['createDirs','remove'].map(n=>[n,['path : String'],'void or Error']),
 ...['rename','copy'].map(n=>[n,['source : String','destination : String'],'void or Error']), ['tempDir',[],'String or Error']]) native('std.fs',name,params,result,true);
for (const [name, params, result] of [['args',[],'List[String]'],['env',['name : String'],'Option[String]'],['cwd',[],'String'],['exit',['code : Int'],'void'],['run',['command : String'],'(Int, String) or Error']]) native('std.process',name,params,result,true);
for (const [name, params, result] of [['now',[],'double'],['monotonic',[],'Int'],['sleep',['seconds : double'],'void']]) native('std.time',name,params,result,true);
for (const [name, params, result] of [['write',['text : String'],'void'],['eprint',['text : String'],'void'],['flush',[],'void'],['input',['prompt : String'],'String or Error']]) native('std.io',name,params,result,true);
for (const [name, params, result] of [['fromList',['values : List[Int]'],'Bytes or Error'],['fromUtf8',['text : String'],'Bytes'],['decodeUtf8',['data : Bytes'],'String or Error'],['toList',['data : Bytes'],'List[Int]'],['toHex',['data : Bytes'],'String']]) native('std.bytes',name,params,result);
for (const [name, params, result] of [['assertError',['value : T'],'void'],['assertTrue',['value : Bool'],'void'],['assertFalse',['value : Bool'],'void'],['assertNotEq',['a : T','b : T'],'void'],['assertNear',['a : double','b : double','tolerance : double'],'void']]) native('std.test',name,params,result);
function method(receiver,name,parameters,result) { add({receiver,name,parameters,signature:`${receiver}.${name}(${parameters.join(', ')}) : ${result}`,module:'std.core'}); }
for(const [name,params,result] of [['substring',['start : Int','end : Int'],'String'],['slice',['start : Int','end : Int'],'String'],['reversed',[],'String'],['ord',[],'Int'],['byteLength',[],'Int'],['byteAt',['at : Int'],'Int'],['byteSlice',['start : Int','end : Int'],'String'],['lines',[],'List[String]'],['words',[],'List[String]'],['padStart',['width : Int','fill : String = " "'],'String'],['padEnd',['width : Int','fill : String = " "'],'String'],['bytes',[],'Bytes'],['nextCharBoundary',['at : Int'],'Int'],['nextGraphemeBoundary',['at : Int'],'Int']]) method('String',name,params,result);
for(const [name,params,result] of [['length',[],'Int'],['isEmpty',[],'Bool'],['get',['at : Int'],'Option[Int]'],['at',['at : Int'],'Int'],['slice',['start : Int','end : Int'],'Bytes'],['concat',['other : Bytes'],'Bytes'],['toList',[],'List[Int]'],['toHex',[],'String'],['decodeUtf8',[],'String or Error']]) method('Bytes',name,params,result);
method('Error','context',['message : String'],'Error'); method('Error','cause',[],'Option[Error]');
add({name:'Bytes',type:true,module:'std.core',signature:'Bytes(values : List[Int]) : Bytes or Error'});
const output = { entries, modules: [...moduleMap].map(([path,names])=>({path,names:[...names].sort()})) };
const target = path.join(root, 'editors/vscode-kestrel/out/library_metadata.json');
const serialized = JSON.stringify(output,null,2)+'\n';
if(process.argv.includes('--check')) { if(fs.readFileSync(target,'utf8') !== serialized) { console.error('Standard-library metadata is stale. Run node tools/generate_editor_metadata.js'); process.exitCode=1; } }
else fs.writeFileSync(target,serialized);

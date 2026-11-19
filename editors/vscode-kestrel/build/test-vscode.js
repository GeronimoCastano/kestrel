'use strict';
const fs=require('fs'),os=require('os'),path=require('path'),net=require('net');
const {spawn}=require('child_process');
const {chromium}=require('playwright-core');
const assert=require('assert');
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
(async()=>{
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'kestrel-vscode-qa-'));
  const server=net.createServer();await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  const port=server.address().port;await new Promise(resolve=>server.close(resolve));
  const executable=process.env.VSCODE_EXECUTABLE || (process.platform==='darwin'?'/Applications/Visual Studio Code.app/Contents/MacOS/Code':'code');
  const env={...process.env,KESTREL_VSCODE_QA:root};delete env.ELECTRON_RUN_AS_NODE;
  const child=spawn(executable,[`--user-data-dir=${root}/profile`,`--extensions-dir=${root}/extensions`,`--remote-debugging-port=${port}`,
    '--disable-workspace-trust','--skip-welcome','--skip-release-notes',`--extensionDevelopmentPath=${process.env.KESTREL_EXTENSION_UNDER_TEST || path.resolve(__dirname,'..')}`,
    `--extensionTestsPath=${path.resolve(__dirname,'../test/vscode')}`,root],{env,stdio:['ignore','pipe','pipe']});
  const log=fs.createWriteStream(path.join(root,'vscode.log'));child.stdout.pipe(log);child.stderr.pipe(log);
  let exited=false;child.on('exit',()=>{exited=true;});child.on('error',error=>{console.error(error);exited=true;});let browser;
  try {
    const deadline=Date.now()+120000;
    while(Date.now()<deadline&&!exited){try{browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);break;}catch(_){await pause(300);}}
    assert(browser,'VS Code debugging connection');
    for(const phase of ['light','dark','large','override','small','zoom','hc-dark','hc-light']){
      while(!fs.existsSync(path.join(root,`${phase}.ready`))&&Date.now()<deadline&&!exited) await pause(100);
      const resultFile=path.join(root,'api-result.json');
      if(fs.existsSync(resultFile)&&!JSON.parse(fs.readFileSync(resultFile)).passed)throw Error(fs.readFileSync(resultFile,'utf8'));
      assert(fs.existsSync(path.join(root,`${phase}.ready`)),`${phase} hover ready`);
      const page=browser.contexts()[0].pages().find(page=>page.url().includes('workbench'));
      assert(page,'editor renderer');
      await page.waitForFunction(()=>[...document.querySelectorAll('.monaco-hover img')].some(img=>img.src.includes('doc-math-v2')&&img.complete&&img.naturalWidth>0),null,{timeout:15000});
      const images=await page.locator('.monaco-hover img').evaluateAll(images=>images.map(img=>{
        const marker=document.createElement('span');
        marker.style.cssText='display:inline-block;width:0;height:0;margin:0;padding:0;border:0;vertical-align:baseline';
        img.before(marker);
        const baseline=marker.getBoundingClientRect().top;
        const rect=img.getBoundingClientRect();
        marker.remove();
        return {src:img.src,width:img.naturalWidth,height:img.naturalHeight,top:rect.top,renderHeight:rect.height,
          baseline,inline:img.getAttribute('align')==='center',alt:img.alt,fontSize:getComputedStyle(img.parentElement).fontSize,align:getComputedStyle(img).verticalAlign};
      }));
      fs.writeFileSync(path.join(root,`${phase}.images.json`),JSON.stringify(images,null,2));
      await page.screenshot({path:path.join(root,`${phase}.png`)});
      for(const image of images.filter(image=>image.inline)) {
        assert.strictEqual(image.align,'middle','native middle alignment');
        const expected=JSON.parse(fs.readFileSync(path.join(root,`${phase}.ready`))).fontSize;
        assert.strictEqual(parseFloat(image.fontSize),expected,'scoped editor font size');
        const svg=fs.readFileSync(decodeURIComponent(new URL(image.src).pathname),'utf8');
        const box=/viewBox="([^"]+)"/.exec(svg)[1].split(/\s+/).map(Number);
        const mathBaseline=image.top-box[1]/box[3]*image.renderHeight;
        assert(Math.abs(mathBaseline-image.baseline)<1.25,`${phase}: baseline for ${image.alt} differs by ${mathBaseline-image.baseline}px`);
        if(image.alt.includes('= \\infty')) assert(svg.includes('data-c="3D"')&&svg.includes('data-c="221E"'),'complete equation');
      }
      assert(images.some(image=>image.alt.includes('= \\infty')),'relation image');
      assert(images.length>=2,`${phase}: inline and display equations`);
      assert(images.every(img=>img.width>0&&img.height>0),`${phase}: all glyph images loaded`);
      fs.writeFileSync(path.join(root,`${phase}.done`),JSON.stringify(images));
    }
    while(!fs.existsSync(path.join(root,'api-result.json'))&&Date.now()<deadline&&!exited)await pause(100);
    const result=JSON.parse(fs.readFileSync(path.join(root,'api-result.json')));assert(result.passed,JSON.stringify(result));
    console.log(`Real VS Code equations and baselines passed across eight theme/font/zoom profiles. Artifacts: ${root}`);
  } finally {if(browser)await browser.close().catch(()=>{});if(!exited)child.kill();log.end();}
})().catch(error=>{console.error(error);process.exitCode=1;});

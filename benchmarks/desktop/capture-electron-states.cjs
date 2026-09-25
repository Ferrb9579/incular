// Separate visual QA launch. No debugging flags are used in memory measurements.
const { spawn } = require('node:child_process');
const { mkdirSync, writeFileSync, openSync, closeSync } = require('node:fs');
const { resolve, join } = require('node:path');
const net = require('node:net');

async function main() {
  const output = resolve(process.argv[2] || 'benchmarks/desktop/results/windows/electron-ui/reference');
  mkdirSync(output, { recursive: true });
  const server = net.createServer();
  await new Promise(done => server.listen(0, '127.0.0.1', done));
  const port = server.address().port;
  await new Promise(done => server.close(done));
  const env = { ...process.env };
  delete env.ELECTRON_CAPTURE;
  delete env.ELECTRON_RUN_AS_NODE;
  const log = openSync(join(output, 'capture.log'), 'w');
  const child = spawn(resolve('target/desktop-benchmark/electron-package/Benchmark Electron-win32-x64/Benchmark Electron.exe'),
    [`--remote-debugging-port=${port}`], { env, windowsHide: true, stdio: ['ignore', log, log] });
  const wait = ms => new Promise(done => setTimeout(done, ms));
  let socket;
  try {
    let target;
    for (let attempt = 0; attempt < 300; attempt++) {
      if (child.exitCode !== null) throw new Error(`Electron exited: ${child.exitCode}`);
      try {
        const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`, { signal: AbortSignal.timeout(1000) })).json();
        target = targets.find(page => page.type === 'page' && page.title === 'Issue tracker — ready');
        if (target) break;
      } catch {}
      await wait(100);
    }
    if (!target) throw new Error('Electron readiness timed out');
    socket = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((done, reject) => { socket.onopen = done; socket.onerror = reject; });
    let sequence = 0;
    const pending = new Map();
    socket.onmessage = event => {
      const message = JSON.parse(event.data);
      const entry = pending.get(message.id);
      if (entry) {
        pending.delete(message.id);
        if (message.error) entry.reject(new Error(JSON.stringify(message.error)));
        else entry.resolve(message.result);
      }
    };
    function command(method, params = {}) {
      return new Promise((resolve, reject) => {
        const id = ++sequence;
        pending.set(id, { resolve, reject });
        socket.send(JSON.stringify({ id, method, params }));
      });
    }
    async function evaluate(expression) {
      const result = await command('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
      return result.result.value;
    }
    const viewport = await evaluate('({width:innerWidth,height:innerHeight,scale:devicePixelRatio,rows:document.querySelectorAll(".issue-row").length})');
    if (viewport.width !== 1100 || viewport.height !== 720 || viewport.rows !== 100) throw new Error(JSON.stringify(viewport));
    async function capture(name) {
      await evaluate('new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))');
      const result = await command('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
      writeFileSync(join(output, `${name}.png`), Buffer.from(result.data, 'base64'));
    }
    await capture('issue-tracker');
    await evaluate('document.querySelector("[data-filter=Completed]").click()');
    await capture('completed');
    await evaluate('document.querySelector("[data-filter=\\"All issues\\"]").click();document.querySelector(".details").scrollTop=100000;document.querySelector("#complete").click()');
    await capture('complete');
    await evaluate('document.querySelector("#complete").click();document.querySelector("#search").value="  APP-2000  ";document.querySelector("#search").dispatchEvent(new Event("input",{bubbles:true}));document.querySelector(".issue-row").click()');
    await capture('search');
    await evaluate('document.querySelector("#search").focus();document.querySelector("#search").value="no-matching-issue";document.querySelector("#search").dispatchEvent(new Event("input",{bubbles:true}))');
    await capture('empty');
    writeFileSync(join(output, 'viewport.json'), JSON.stringify(viewport, null, 2));
    await command('Browser.close');
  } finally {
    socket?.close();
    if (child.exitCode === null) child.kill();
    closeSync(log);
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });

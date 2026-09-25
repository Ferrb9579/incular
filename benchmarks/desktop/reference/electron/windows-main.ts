// Windows host adaptation of electron/main.ts (QuickGUI MIT benchmark).
// Keep its workload, sandbox and single BrowserWindow. Correct the actual
// CSS viewport before loading it: Windows DPI/non-client rounding on this
// host makes useContentSize alone produce 1100 x 722 rather than 1100 x 720.
import { app, BrowserWindow } from "electron";
import { join } from "node:path";
import { writeFileSync } from "node:fs";

app.whenReady().then(async () => {
  const window = new BrowserWindow({
    title: "Loading issue tracker", width: 1100, height: 720,
    useContentSize: true, backgroundColor: "#ffffff",
    webPreferences: { contextIsolation: true, nodeIntegration: false, sandbox: true },
  });
  window.removeMenu();
  await window.loadURL("about:blank");
  for (let attempt = 0; attempt < 5; attempt++) {
    const [width, height] = await window.webContents.executeJavaScript("[innerWidth, innerHeight]");
    if (width === 1100 && height === 720) break;
    const [contentWidth, contentHeight] = window.getContentSize();
    window.setContentSize(contentWidth + 1100 - width, contentHeight + 720 - height);
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  await window.loadFile(join(app.getAppPath(), "index.html"));
  // Separate validation launch only; never enabled during memory sampling.
  if (process.env.ELECTRON_CAPTURE) {
    for (let attempt = 0; attempt < 50; attempt++) {
      const state = await window.webContents.executeJavaScript(
        "({title:document.title, rows:document.querySelectorAll('.issue-row').length, width:innerWidth, height:innerHeight})"
      );
      if (state.title === "Issue tracker — ready") {
        if (state.rows !== 100 || state.width !== 1100 || state.height !== 720) throw new Error(JSON.stringify(state));
        writeFileSync(process.env.ELECTRON_CAPTURE, (await window.webContents.capturePage()).toPNG());
        console.log("Validated upstream readiness, 100 retained rows, 1100 x 720 viewport");
        app.quit();
        return;
      }
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    throw new Error("Electron capture readiness timed out");
  }
}).catch(error => { console.error(error); app.exit(1); });
app.on("window-all-closed", () => app.quit());

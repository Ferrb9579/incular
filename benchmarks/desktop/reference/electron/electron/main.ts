import { app, BrowserWindow } from "electron";
import { join } from "node:path";

app
  .whenReady()
  .then(async () => {
    const window = new BrowserWindow({
      title: "Loading issue tracker",
      width: 1100,
      height: 720,
      useContentSize: true,
      backgroundColor: "#ffffff",
      webPreferences: { contextIsolation: true, nodeIntegration: false, sandbox: true },
    });
    // Bun resolves __dirname at bundle time. Electron's app path points inside
    // the installed ASAR, so this also works without the source checkout.
    await window.loadFile(join(app.getAppPath(), "index.html"));
  })
  .catch((error) => {
    console.error("Could not load the benchmark issue tracker:", error);
    app.exit(1);
  });
app.on("window-all-closed", () => app.quit());

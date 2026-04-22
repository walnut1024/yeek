import { app, BrowserWindow, protocol, net } from "electron";
import { ChildProcess, spawn } from "child_process";
import path from "path";
import http from "http";

const SERVER_PORT = 17321;
const VITE_DEV_URL = "http://localhost:1420";
const READINESS_RETRIES = 20;
const READINESS_INTERVAL_MS = 500;

let serverProcess: ChildProcess | null = null;
let mainWindow: BrowserWindow | null = null;

// Register custom protocol before app is ready (required by Electron)
protocol.registerSchemesAsPrivileged([
  {
    scheme: "yeek",
    privileges: {
      secure: true,
      standard: true,
      supportFetchAPI: true,
      corsEnabled: true,
    },
  },
]);

function getServerPath(): string {
  if (app.isPackaged) {
    return path.join(process.resourcesPath, "yeek-server");
  }
  return path.resolve(__dirname, "..", "..", "src-tauri", "target", "debug", "yeek-server");
}

function startServer(): ChildProcess {
  const serverPath = getServerPath();
  console.log(`[yeek-electron] Starting server: ${serverPath}`);

  const proc = spawn(serverPath, [], { stdio: ["ignore", "pipe", "pipe"] });

  proc.stdout?.on("data", (data: Buffer) => {
    console.log(`[yeek-server] ${data.toString().trim()}`);
  });
  proc.stderr?.on("data", (data: Buffer) => {
    console.error(`[yeek-server] ${data.toString().trim()}`);
  });
  proc.on("error", (err) => {
    console.error("[yeek-electron] Failed to start yeek-server:", err);
    app.quit();
  });

  return proc;
}

function waitForServer(): Promise<void> {
  return new Promise((resolve, reject) => {
    let attempts = 0;
    const tryConnect = () => {
      if (attempts >= READINESS_RETRIES) {
        reject(new Error(`Server not ready after ${(READINESS_RETRIES * READINESS_INTERVAL_MS) / 1000}s`));
        return;
      }
      attempts++;
      const req = http.get(
        `http://127.0.0.1:${SERVER_PORT}/api/system/status`,
        (res) => {
          if (res.statusCode === 200) {
            res.resume();
            resolve();
          } else {
            setTimeout(tryConnect, READINESS_INTERVAL_MS);
          }
        },
      );
      req.on("error", () => setTimeout(tryConnect, READINESS_INTERVAL_MS));
      req.setTimeout(2000, () => {
        req.destroy();
        setTimeout(tryConnect, READINESS_INTERVAL_MS);
      });
    };
    tryConnect();
  });
}

function registerProtocol() {
  protocol.handle("yeek", (request) => {
    const url = new URL(request.url);
    const filePath = path.join(__dirname, "..", "dist", url.pathname);
    return net.fetch(`file://${filePath}`);
  });
}

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    title: "Yeek",
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  if (app.isPackaged) {
    mainWindow.loadURL("yeek://localhost/index.html");
  } else {
    mainWindow.loadURL(VITE_DEV_URL);
  }

  mainWindow.on("closed", () => {
    mainWindow = null;
  });
}

function killServer() {
  if (serverProcess) {
    serverProcess.kill();
    serverProcess = null;
  }
}

app.on("ready", async () => {
  try {
    registerProtocol();
    serverProcess = startServer();
    console.log("[yeek-electron] Waiting for server...");
    await waitForServer();
    console.log("[yeek-electron] Server ready, creating window");
    createWindow();
  } catch (err) {
    console.error("[yeek-electron] Startup failed:", err);
    killServer();
    app.quit();
  }
});

app.on("window-all-closed", () => {
  killServer();
  if (process.platform !== "darwin") {
    app.quit();
  }
});

app.on("activate", () => {
  if (mainWindow === null) {
    createWindow();
  }
});

app.on("before-quit", () => {
  killServer();
});

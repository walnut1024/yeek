import { app, BrowserWindow, protocol, net } from "electron";
import { ChildProcess, spawn } from "child_process";
import path from "path";
import fs from "fs";
import http from "http";

const SERVER_PORT = 17321;
const VITE_DEV_URL = "http://localhost:1420";
const READINESS_RETRIES = 20;
const READINESS_INTERVAL_MS = 500;
const LOG_FILE = path.join(app.getPath("home"), ".yeek", "electron.log");

let serverProcess: ChildProcess | null = null;
let mainWindow: BrowserWindow | null = null;

function log(msg: string) {
  const line = `[${new Date().toISOString()}] ${msg}\n`;
  console.log(line.trimEnd());
  try {
    fs.mkdirSync(path.dirname(LOG_FILE), { recursive: true });
    fs.appendFileSync(LOG_FILE, line);
  } catch {}
}

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
  log(`Starting server: ${serverPath}`);
  log(`isPackaged: ${app.isPackaged}`);
  log(`resourcesPath: ${process.resourcesPath}`);
  log(`binary exists: ${fs.existsSync(serverPath)}`);
  log(`__dirname: ${__dirname}`);

  // Electron patches child_process.spawn to resolve paths inside asar archives.
  // Use the original Node.js spawn by requiring it directly from process.execPath.
  const proc = spawn(serverPath, [], {
    stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env },
  });

  proc.stdout?.on("data", (data: Buffer) => {
    log(`[server:out] ${data.toString().trim()}`);
  });
  proc.stderr?.on("data", (data: Buffer) => {
    log(`[server:err] ${data.toString().trim()}`);
  });
  proc.on("error", (err) => {
    log(`Failed to start yeek-server: ${err.message}`);
    app.quit();
  });
  proc.on("exit", (code, signal) => {
    log(`yeek-server exited: code=${code} signal=${signal}`);
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
    // __dirname = app.asar/electron-app/dist — go up 2 levels to reach app.asar root
    const filePath = path.join(__dirname, "..", "..", "dist", url.pathname);
    log(`protocol handle: ${request.url} -> file://${filePath}`);
    try {
      const data = fs.readFileSync(filePath);
      const ext = path.extname(filePath);
      const mime =
        ext === ".html" ? "text/html" :
        ext === ".js" ? "text/javascript" :
        ext === ".css" ? "text/css" :
        ext === ".svg" ? "image/svg+xml" :
        ext === ".json" ? "application/json" :
        "application/octet-stream";
      return new Response(data, {
        headers: { "content-type": `${mime}; charset=utf-8` },
      });
    } catch (err: any) {
      log(`protocol error: ${err.message}`);
      return new Response("Not Found", { status: 404 });
    }
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
    log("Waiting for server...");
    await waitForServer();
    log("Server ready, creating window");
    createWindow();
  } catch (err) {
    log(`Startup failed: ${err}`);
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

import { NextResponse } from "next/server";
import { homedir } from "os";
import { join } from "path";
import { readFileSync, existsSync } from "fs";

const CONFIG_PATH = join(homedir(), ".portal.json");

interface PortalInstance {
  id: string;
  name: string;
  directory: string;
  port: number | null;
  opencodePort: number;
  hostname: string;
  opencodePid: number | null;
  webPid: number | null;
  startedAt: string;
  instanceType: string;
  containerId: string | null;
}

function readConfig() {
  try {
    if (existsSync(CONFIG_PATH)) {
      const content = readFileSync(CONFIG_PATH, "utf-8");
      const config = JSON.parse(content);
      config.instances = (config.instances || []).map((instance: PortalInstance) => ({
        ...instance,
        instanceType: instance.instanceType || "process",
        containerId: instance.containerId || null,
        opencodePid: instance.opencodePid ?? null,
        webPid: instance.webPid ?? null,
      }));
      return config;
    }
  } catch {}
  return { instances: [] };
}

function isProcessRunning(pid: number | null): boolean {
  if (pid === null) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

export async function GET() {
  const config = readConfig();
  const instances = config.instances
    .filter((instance: PortalInstance) => {
      const opencodeRunning = isProcessRunning(instance.opencodePid);
      const webRunning = isProcessRunning(instance.webPid);
      return opencodeRunning || webRunning;
    })
    .map((instance: PortalInstance) => ({
      id: instance.id,
      name: instance.name,
      directory: instance.directory,
      port: instance.opencodePort,
      hostname: instance.hostname,
      opencodePid: instance.opencodePid,
      webPid: instance.webPid,
      startedAt: instance.startedAt,
      instanceType: instance.instanceType,
      containerId: instance.containerId,
      state: "running" as const,
      status: `Running since ${new Date(instance.startedAt).toLocaleString()}`,
    }));
  return NextResponse.json({ total: instances.length, instances });
}

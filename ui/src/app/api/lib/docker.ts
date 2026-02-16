export async function isContainerRunning(containerId: string | null): Promise<boolean> {
  if (!containerId) return false;
  try {
    const { execSync } = require("child_process");
    const result = execSync(`docker inspect -f '{{.State.Running}}' ${containerId}`, { encoding: "utf-8" }).trim();
    return result === "true";
  } catch {
    return false;
  }
}

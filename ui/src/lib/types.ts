// NOTE(victor): Must stay in sync with daemon/src/nodes.rs NodeInfo
export interface Node {
  id: string;
  name: string;
  url: string;
  startedAt: string;
  os: string;
  arch: string;
  daemonVersion: string;
  machineName?: string;
}

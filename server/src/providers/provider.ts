export interface VMProvider {
  create(displayName?: string): Promise<{ name: string; nodeId: string }>;
  destroy(name: string): Promise<void>;
  list(): Promise<Array<{ name: string; status?: string }>>;
  injectProxyAuth(targetUrl: URL, headers: Headers): void;
}

export function getServerUrl(): string {
  if (process.env.NODE_ENV === "production") {
    const url = process.env.SERVER_URL;
    if (!url) throw new Error("SERVER_URL not set");
    return url;
  }
  return "https://nightshift-server.fly.dev";
}

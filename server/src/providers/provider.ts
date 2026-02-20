export interface VMProvider {
  create(displayName?: string): Promise<{ name: string; nodeId: string }>;
  destroy(name: string): Promise<void>;
  list(): Promise<Array<{ name: string; status?: string }>>;
  injectProxyAuth(targetUrl: URL, headers: Headers): void;
}

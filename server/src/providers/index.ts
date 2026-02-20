import type { VMProvider } from "./provider";
import { spritesProvider } from "./sprites";
import { digitalOceanProvider } from "./digitalocean";

const providers: Record<string, VMProvider> = {
  sprites: spritesProvider,
  digitalocean: digitalOceanProvider,
};

export function getProvider(): VMProvider {
  const name = process.env.NIGHTSHIFT_PROVIDER ?? "sprites";
  const provider = providers[name];
  if (!provider) {
    throw new Error(`unknown provider: ${name} (available: ${Object.keys(providers).join(", ")})`);
  }
  return provider;
}

export type { VMProvider } from "./provider";

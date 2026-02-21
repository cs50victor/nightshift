"use client";
import { useEffect, useMemo } from "react";
import {
  Autocomplete,
  ListBox,
  Popover,
  useFilter,
} from "react-aria-components";
import { Dialog } from "@/components/ui/dialog";
import { SearchField, SearchInput } from "@/components/ui/search-field";
import {
  Select,
  SelectItem,
  SelectSection,
  SelectTrigger,
} from "@/components/ui/select";
import { useConfigStore } from "@/stores/config-store";
import { useModelStore } from "@/stores/model-store";

interface ModelItem {
  id: string;
  name: string;
}

interface ProviderWithModels {
  id: string;
  name: string;
  models: ModelItem[];
}

interface ModelSelectProps {
  triggerClassName?: string;
}

export function ModelSelect({ triggerClassName }: ModelSelectProps = {}) {
  const rawProviders = useConfigStore((s) => s.providers);
  const defaultModel = useConfigStore((s) => s.defaultModel);
  const { contains } = useFilter({ sensitivity: "base" });

  const selectedModel = useModelStore((s) => s.selectedModel);
  const setModelFromKey = useModelStore((s) => s.setModelFromKey);
  const setModelFromDefault = useModelStore((s) => s.setModelFromDefault);

  const providers: ProviderWithModels[] = useMemo(
    () =>
      rawProviders.map((provider) => ({
        id: provider.id,
        name: provider.name,
        models: Object.values(provider.models || {}).map((model) => ({
          id: `${provider.id}/${model.id}`,
          name: model.name,
        })),
      })),
    [rawProviders],
  );
  const selectedModelKey =
    selectedModel.providerID && selectedModel.modelID
      ? `${selectedModel.providerID}/${selectedModel.modelID}`
      : null;

  useEffect(() => {
    if (defaultModel) {
      setModelFromDefault(defaultModel);
    }
  }, [defaultModel, setModelFromDefault]);

  return (
    <Select
      aria-label="Model"
      placeholder={
        rawProviders.length === 0 ? "Loading models..." : "Select a model"
      }
      className="w-auto"
      selectedKey={selectedModelKey}
      onSelectionChange={(key) => {
        if (key) {
          setModelFromKey(String(key));
        }
      }}
    >
      <SelectTrigger className={triggerClassName ?? "w-48"} />
      <Popover className="entering:fade-in exiting:fade-out flex max-h-96 w-(--trigger-width) entering:animate-in exiting:animate-out flex-col overflow-hidden rounded-lg border bg-overlay">
        <Dialog aria-label="Model">
          <Autocomplete filter={contains}>
            <div className="border-b bg-muted p-2">
              <SearchField className="rounded-lg bg-bg" autoFocus>
                <SearchInput placeholder="Search models..." />
              </SearchField>
            </div>
            <ListBox
              className="grid max-h-80 w-full grid-cols-[auto_1fr] flex-col gap-y-1 overflow-y-auto p-1 outline-hidden *:[[role='group']+[role=group]]:mt-4 *:[[role='group']+[role=separator]]:mt-1"
              items={providers}
            >
              {(provider) => (
                <SelectSection title={provider.name} items={provider.models}>
                  {(model) => (
                    <SelectItem id={model.id} textValue={model.name}>
                      {model.name}
                    </SelectItem>
                  )}
                </SelectSection>
              )}
            </ListBox>
          </Autocomplete>
        </Dialog>
      </Popover>
    </Select>
  );
}

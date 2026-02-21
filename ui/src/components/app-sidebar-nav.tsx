"use client";
import { EllipsisHorizontalIcon } from "@heroicons/react/24/outline";
import { PlusIcon } from "@heroicons/react/24/solid";
import { Breadcrumbs, BreadcrumbsItem } from "@/components/ui/breadcrumbs";
import { Button } from "@/components/ui/button";
import { useBreadcrumb } from "@/contexts/breadcrumb-context";
import { useCreateSession } from "@/lib/use-create-session";
import { NodeSelect } from "./node-select";

export function AppSidebarNav() {
  const { pageTitle } = useBreadcrumb();
  const { creating, handleNewSession } = useCreateSession();

  return (
    <nav className="sticky top-0 z-10 flex items-center justify-between border-b bg-bg px-4 py-2">
      <Breadcrumbs>
        <BreadcrumbsItem href="/">nightshift</BreadcrumbsItem>
        {pageTitle && <BreadcrumbsItem>{pageTitle}</BreadcrumbsItem>}
      </Breadcrumbs>
      <span className="flex items-center gap-x-2 ml-auto">
        <NodeSelect />
        <Button
          size="xs"
          intent="outline"
          className="uppercase font-mono text-muted-fg hover:text-fg"
          onPress={handleNewSession}
          isDisabled={creating}
        >
          <PlusIcon className="size-3.5" />
          {creating ? "Creating..." : "New Session"}
        </Button>
        <Button
          size="xs"
          intent="outline"
          className="font-mono text-muted-fg hover:text-fg"
          onPress={() =>
            window.dispatchEvent(new CustomEvent("open-command-menu"))
          }
        >
          <EllipsisHorizontalIcon className="size-4" />
        </Button>
      </span>
    </nav>
  );
}

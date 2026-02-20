"use client";
import { EllipsisHorizontalIcon } from "@heroicons/react/24/outline";
import { PlusIcon } from "@heroicons/react/24/solid";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { Breadcrumbs, BreadcrumbsItem } from "@/components/ui/breadcrumbs";
import { Button } from "@/components/ui/button";
import { toast } from "@/components/ui/toast";
import { useBreadcrumb } from "@/contexts/breadcrumb-context";
import { useCreateSession } from "@/hooks/use-opencode";
import { NodeSelect } from "./node-select";

export function AppSidebarNav() {
  const { pageTitle } = useBreadcrumb();
  const [isCreatingSession, setIsCreatingSession] = useState(false);
  const router = useRouter();
  const createSession = useCreateSession();

  const handleNewSession = async () => {
    setIsCreatingSession(true);
    try {
      const session = await createSession();
      toast.success("Session created");
      router.push(`/session/${session.id}`);
    } catch (err) {
      console.error("Failed to create session:", err);
      toast.error("Failed to create session");
    } finally {
      setIsCreatingSession(false);
    }
  };

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
          isDisabled={isCreatingSession}
        >
          <PlusIcon className="size-3.5" />
          {isCreatingSession ? "Creating..." : "New Session"}
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

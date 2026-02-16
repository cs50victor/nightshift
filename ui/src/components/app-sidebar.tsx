"use client";
import { CubeIcon, DocumentTextIcon } from "@heroicons/react/24/outline";
import { PlusIcon } from "@heroicons/react/24/solid";
import { parsePatchFiles } from "@pierre/diffs";
import { useRouter } from "next/navigation";
import { useMemo, useState } from "react";
import { Link as UILink } from "@/components/ui/link";
import {
  Sidebar,
  SidebarContent,
  SidebarHeader,
  SidebarItem,
  SidebarLabel,
  SidebarRail,
  SidebarSection,
  SidebarSectionGroup,
} from "@/components/ui/sidebar";
import { toast } from "@/components/ui/toast";
import {
  useCreateSession,
  useCurrentProject,
  useGitDiff,
} from "@/hooks/use-opencode";

interface Project {
  id: string;
  worktree: string;
  vcs?: string;
  time?: {
    created?: number;
    initialized?: number;
    updated?: number;
  };
}

function getProjectName(worktree: string): string {
  const parts = worktree.split("/");
  return parts[parts.length - 1] || worktree;
}

function CurrentProject() {
  const { data: currentProject } = useCurrentProject() as {
    data: Project | undefined;
  };

  const projectName = currentProject
    ? getProjectName(currentProject.worktree)
    : "Loading...";

  return (
    <div className="flex items-center gap-2 px-2 py-1.5">
      <CubeIcon className="size-[18px] shrink-0 text-muted-fg" />
      <div className="text-sm font-medium">{projectName}</div>
    </div>
  );
}

export default function AppSidebar(
  props: React.ComponentProps<typeof Sidebar>,
) {
  const [creating, setCreating] = useState(false);
  const router = useRouter();
  const createSession = useCreateSession();

  const { data: diffData } = useGitDiff();
  const diffFileCount = useMemo(() => {
    if (!diffData?.diff) return 0;
    try {
      const patches = parsePatchFiles(diffData.diff);
      return patches.reduce((count, patch) => count + patch.files.length, 0);
    } catch {
      return 0;
    }
  }, [diffData?.diff]);

  async function handleNewSession() {
    if (creating) return;
    setCreating(true);
    try {
      const session = await createSession();
      toast.success("Session created");
      router.push(`/session/${session.id}`);
    } catch (error) {
      console.error("Failed to create session:", error);
      toast.error("Failed to create session");
    } finally {
      setCreating(false);
    }
  }

  return (
    <Sidebar {...props}>
      <SidebarHeader>
        <UILink href="/" className="flex items-center gap-x-2">
          {/* biome-ignore lint/performance/noImgElement: static SVG logo, no optimization needed */}
          <img src="/logo.svg" alt="OpenCode Portal" className="size-6" />
          <SidebarLabel className="font-medium">
            OpenCode <span className="text-muted-fg">Portal</span>
          </SidebarLabel>
        </UILink>
      </SidebarHeader>
      <SidebarContent>
        <SidebarSectionGroup>
          <SidebarSection>
            <CurrentProject />
          </SidebarSection>

          <SidebarSection>
            <SidebarItem
              tooltip="New Session"
              onPress={handleNewSession}
              className="cursor-pointer gap-x-2"
            >
              <PlusIcon className="size-4 shrink-0" data-slot="icon" />
              <SidebarLabel>
                {creating ? "Creating..." : "New Session"}
              </SidebarLabel>
            </SidebarItem>
            <SidebarItem
              tooltip="View Git Diff"
              href="/diff"
              className="cursor-pointer gap-x-2"
              badge={diffFileCount > 0 ? diffFileCount : undefined}
            >
              <DocumentTextIcon className="size-4 shrink-0" data-slot="icon" />
              <SidebarLabel>Diff</SidebarLabel>
            </SidebarItem>
          </SidebarSection>
        </SidebarSectionGroup>
      </SidebarContent>

      <SidebarRail />
    </Sidebar>
  );
}

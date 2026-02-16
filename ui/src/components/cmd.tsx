"use client";
import {
  ArrowDownCircleIcon,
  ArrowPathIcon,
  ArrowUpCircleIcon,
  ComputerDesktopIcon,
  DocumentTextIcon,
  MoonIcon,
  SquaresPlusIcon,
  SunIcon,
} from "@heroicons/react/24/outline";
import { TrashIcon } from "@heroicons/react/24/solid";
import { useParams, usePathname, useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import {
  CommandMenu,
  CommandMenuItem,
  CommandMenuLabel,
  CommandMenuList,
  CommandMenuSearch,
  CommandMenuSection,
  CommandMenuSeparator,
} from "@/components/ui/command-menu";
import { toast } from "@/components/ui/toast";
import {
  useCreateSession,
  useDeleteSession,
  useSessions,
} from "@/hooks/use-opencode";
import { mutateSessionMessages } from "@/hooks/use-session-messages";
import { useModelStore } from "@/stores/model-store";
import { useTheme } from "@/providers/theme-provider";

const CREATE_PR_PROMPT = `Use gh CLI to create a pull request. Follow these steps:

1. First, check git status to see all changes
2. Stage all relevant changes with git add
3. Get the diff of staged changes
4. Generate a clear, descriptive commit message based on the changes
5. Commit the changes
6. Push to the remote branch (create branch if needed)
7. Create a PR using gh pr create with a descriptive title and body
8. After the PR is created, checkout to main branch

Make sure to:
- Write a meaningful commit message that explains WHY, not just WHAT
- The PR title should be concise but descriptive
- The PR body should summarize the changes and their purpose
- Always checkout to main after successfully creating the PR`;

const PULL_CHANGES_PROMPT = `Pull the latest changes from the remote repository. Follow these steps:

1. First, check git status to see if there are any uncommitted changes
2. If there are uncommitted changes, stash them with a descriptive message
3. Run git pull to fetch and merge the latest changes from the remote
4. If there were stashed changes, pop the stash and resolve any conflicts if needed
5. Show a summary of what was pulled (new commits, files changed)

Make sure to:
- Handle any merge conflicts gracefully
- Report what changes were pulled
- Restore any stashed changes after pulling`;

const PUSH_CHANGES_PROMPT = `Push the current changes to the remote repository. Follow these steps:

1. First, check git status to see all uncommitted changes
2. If there are uncommitted changes:
   - Stage all relevant changes with git add
   - Generate a clear, descriptive commit message based on the changes
   - Commit the changes
3. Check if the current branch has an upstream branch set
4. Push to the remote (set upstream if needed)
5. Show a summary of what was pushed

Make sure to:
- Write a meaningful commit message that explains WHY, not just WHAT
- Handle any push rejections (e.g., if remote has new commits, pull first)
- Report the result of the push operation`;

export default function Cmd() {
  const [isOpen, setIsOpen] = useState(false);
  const [creating, setCreating] = useState(false);
  const router = useRouter();
  const pathname = usePathname();
  const params = useParams();
  const { mutate } = useSessions();
  const createSession = useCreateSession();
  const deleteSession = useDeleteSession();
  const selectedModel = useModelStore((s) => s.selectedModel);
  const { setTheme } = useTheme();

  const currentSessionId = params.id as string | undefined;
  const isOnSessionPage = pathname.startsWith("/session/") && currentSessionId;

  useEffect(() => {
    setIsOpen(false);
  }, []);

  useEffect(() => {
    const handler = () => setIsOpen(true);
    window.addEventListener("open-command-menu", handler);
    return () => window.removeEventListener("open-command-menu", handler);
  }, []);

  async function sendPrompt(prompt: string) {
    if (!currentSessionId) {
      toast.error("Please open a session first");
      return;
    }
    const response = await fetch(
      `/api/opencode/session/${currentSessionId}/prompt_async`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          parts: [{ type: "text", text: prompt }],
          model: selectedModel,
        }),
      },
    );
    if (!response.ok) throw new Error("Failed to send request");
    mutateSessionMessages(currentSessionId);
    mutate();
  }

  async function handleNewSession() {
    setCreating(true);
    setIsOpen(false);
    try {
      const newSession = await createSession();
      await mutate();
      toast.success("Session created");
      router.push(`/session/${newSession.id}`);
    } catch (err) {
      console.error("Failed to create session:", err);
      toast.error("Failed to create session");
    } finally {
      setCreating(false);
    }
  }

  async function handleDeleteSession() {
    if (!currentSessionId) return;

    setIsOpen(false);
    try {
      await deleteSession(currentSessionId);
      await mutate();
      toast.success("Session deleted");
      router.push("/");
    } catch (err) {
      console.error("Failed to delete session:", err);
      toast.error("Failed to delete session");
    }
  }

  function handleThemeChange(theme: "light" | "dark" | "system") {
    setTheme(theme);
    setIsOpen(false);
  }

  return (
    <CommandMenu
      isOpen={isOpen}
      onOpenChange={setIsOpen}
      shortcut="k"
      isBlurred
    >
      <CommandMenuSearch placeholder="Search commands..." />
      <CommandMenuList>
        {isOnSessionPage && (
          <CommandMenuSection label="Session Actions">
            <CommandMenuItem
              textValue="Delete current session"
              intent="danger"
              onAction={handleDeleteSession}
            >
              <TrashIcon className="size-4" />
              <CommandMenuLabel>Delete Current Session</CommandMenuLabel>
            </CommandMenuItem>
          </CommandMenuSection>
        )}

        {isOnSessionPage && <CommandMenuSeparator />}

        <CommandMenuSection label="Actions">
          <CommandMenuItem
            textValue="New session"
            onAction={handleNewSession}
            isDisabled={creating}
          >
            <SquaresPlusIcon className="size-4 mr-2" />
            <CommandMenuLabel>
              {creating ? "Creating..." : "New Session"}
            </CommandMenuLabel>
          </CommandMenuItem>
          <CommandMenuItem
            textValue="View diff"
            onAction={() => {
              setIsOpen(false);
              router.push("/diff");
            }}
          >
            <DocumentTextIcon className="size-4 mr-2" />
            <CommandMenuLabel>Diff</CommandMenuLabel>
          </CommandMenuItem>
        </CommandMenuSection>

        {isOnSessionPage && <CommandMenuSeparator />}

        {isOnSessionPage && (
          <CommandMenuSection label="Git">
            <CommandMenuItem
              textValue="Pull changes"
              onAction={async () => {
                setIsOpen(false);
                try {
                  await sendPrompt(PULL_CHANGES_PROMPT);
                  toast.success("Pull request sent");
                } catch {
                  toast.error("Failed to send pull request");
                }
              }}
            >
              <ArrowDownCircleIcon className="size-4 mr-2" />
              <CommandMenuLabel>Pull</CommandMenuLabel>
            </CommandMenuItem>
            <CommandMenuItem
              textValue="Push changes"
              onAction={async () => {
                setIsOpen(false);
                try {
                  await sendPrompt(PUSH_CHANGES_PROMPT);
                  toast.success("Push request sent");
                } catch {
                  toast.error("Failed to send push request");
                }
              }}
            >
              <ArrowUpCircleIcon className="size-4 mr-2" />
              <CommandMenuLabel>Push</CommandMenuLabel>
            </CommandMenuItem>
            <CommandMenuItem
              textValue="Create pull request"
              onAction={async () => {
                setIsOpen(false);
                try {
                  await sendPrompt(CREATE_PR_PROMPT);
                  toast.success("PR creation request sent");
                } catch {
                  toast.error("Failed to send PR creation request");
                }
              }}
            >
              <ArrowPathIcon className="size-4 mr-2" />
              <CommandMenuLabel>Create PR</CommandMenuLabel>
            </CommandMenuItem>
          </CommandMenuSection>
        )}

        <CommandMenuSeparator />

        <CommandMenuSection label="Theme">
          <CommandMenuItem
            textValue="Light theme"
            onAction={() => handleThemeChange("light")}
          >
            <SunIcon className="size-4 mr-2" />
            <CommandMenuLabel>Light</CommandMenuLabel>
          </CommandMenuItem>
          <CommandMenuItem
            textValue="Dark theme"
            onAction={() => handleThemeChange("dark")}
          >
            <MoonIcon className="size-4 mr-2" />
            <CommandMenuLabel>Dark</CommandMenuLabel>
          </CommandMenuItem>
          <CommandMenuItem
            textValue="System theme"
            onAction={() => handleThemeChange("system")}
          >
            <ComputerDesktopIcon className="size-4 mr-2" />
            <CommandMenuLabel>System</CommandMenuLabel>
          </CommandMenuItem>
        </CommandMenuSection>
      </CommandMenuList>
    </CommandMenu>
  );
}

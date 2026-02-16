"use client";
import { PlusIcon } from "@heroicons/react/24/solid";
import { useParams, useRouter } from "next/navigation";
import { useState } from "react";
import ArrowDownCircleIcon from "@/components/icons/arrow-down-circle-icon";
import ArrowUpCircleIcon from "@/components/icons/arrow-up-circle-icon";
import FileDiffIcon from "@/components/icons/file-diff-icon";
import IconGitPullRequest from "@/components/icons/git-pull-request-icon";
import { Breadcrumbs, BreadcrumbsItem } from "@/components/ui/breadcrumbs";
import { Button } from "@/components/ui/button";
import { toast } from "@/components/ui/toast";
import { useBreadcrumb } from "@/contexts/breadcrumb-context";
import { useCreateSession, useSessions } from "@/hooks/use-opencode";
import { mutateSessionMessages } from "@/hooks/use-session-messages";
import { useModelStore } from "@/stores/model-store";

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

export function AppSidebarNav() {
  const { pageTitle } = useBreadcrumb();
  const selectedModel = useModelStore((s) => s.selectedModel);
  const { mutate: mutateSessions } = useSessions();

  const [isCreatingSession, setIsCreatingSession] = useState(false);
  const [isCreatingPR, setIsCreatingPR] = useState(false);
  const [isPulling, setIsPulling] = useState(false);
  const [isPushing, setIsPushing] = useState(false);
  const router = useRouter();
  const createSession = useCreateSession();

  const params = useParams();
  const sessionId = params.id as string | undefined;

  const sendPrompt = async (prompt: string) => {
    if (!sessionId) {
      toast.error("Please open a session first");
      return;
    }

    const response = await fetch(`/api/opencode/session/${sessionId}/prompt`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        parts: [{ type: "text", text: prompt }],
        model: selectedModel,
      }),
    });

    if (!response.ok) {
      throw new Error("Failed to send request");
    }

    mutateSessionMessages(sessionId);
    mutateSessions();
  };

  const handleCreatePR = async () => {
    setIsCreatingPR(true);
    try {
      await sendPrompt(CREATE_PR_PROMPT);
      toast.success("PR creation request sent");
    } catch (err) {
      console.error("Failed to create PR:", err);
      toast.error("Failed to send PR creation request");
    } finally {
      setIsCreatingPR(false);
    }
  };

  const handlePull = async () => {
    setIsPulling(true);
    try {
      await sendPrompt(PULL_CHANGES_PROMPT);
      toast.success("Pull request sent");
    } catch (err) {
      console.error("Failed to pull:", err);
      toast.error("Failed to send pull request");
    } finally {
      setIsPulling(false);
    }
  };

  const handlePush = async () => {
    setIsPushing(true);
    try {
      await sendPrompt(PUSH_CHANGES_PROMPT);
      toast.success("Push request sent");
    } catch (err) {
      console.error("Failed to push:", err);
      toast.error("Failed to send push request");
    } finally {
      setIsPushing(false);
    }
  };

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

  const isLoading = isCreatingSession || isCreatingPR || isPulling || isPushing;

  return (
    <nav className="sticky top-0 z-10 flex items-center justify-between border-b bg-bg px-4 py-2">
      <Breadcrumbs>
        <BreadcrumbsItem href="/">Nightshift</BreadcrumbsItem>
        {pageTitle && <BreadcrumbsItem>{pageTitle}</BreadcrumbsItem>}
      </Breadcrumbs>
      <span className="flex items-center gap-x-2 ml-auto">
        <Button
          size="xs"
          intent="outline"
          className="uppercase font-mono text-muted-fg hover:text-fg"
          onPress={handlePull}
          isDisabled={isLoading || !sessionId}
        >
          <ArrowDownCircleIcon size="14px" />
          {isPulling ? "Pulling..." : "Pull"}
        </Button>
        <Button
          size="xs"
          intent="outline"
          className="uppercase font-mono text-muted-fg hover:text-fg"
          onPress={handlePush}
          isDisabled={isLoading || !sessionId}
        >
          <ArrowUpCircleIcon size="14px" />
          {isPushing ? "Pushing..." : "Push"}
        </Button>
        <Button
          size="xs"
          intent="outline"
          className="uppercase font-mono text-muted-fg hover:text-fg"
          onPress={handleCreatePR}
          isDisabled={isLoading || !sessionId}
        >
          <IconGitPullRequest size="14px" />
          {isCreatingPR ? "Creating..." : "Create PR"}
        </Button>
        <Button
          size="xs"
          intent="outline"
          className="uppercase font-mono text-muted-fg hover:text-fg"
          onPress={() => router.push("/diff")}
        >
          <FileDiffIcon className="size-3.5" />
          Diff
        </Button>
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
      </span>
    </nav>
  );
}

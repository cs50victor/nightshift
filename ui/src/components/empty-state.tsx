"use client";
import { SquaresPlusIcon } from "@heroicons/react/24/outline";
import { useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Keyboard } from "@/components/ui/keyboard";
import { useCreateSession } from "@/lib/use-create-session";

export default function EmptyState() {
  const { creating, handleNewSession } = useCreateSession();

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Enter" && event.shiftKey && !creating) {
        event.preventDefault();
        handleNewSession();
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [creating, handleNewSession]);

  return (
    <div className="flex h-full items-center justify-center">
      <div className="text-center max-w-md">
        <p className="text-muted-fg mb-6">
          Create a new session to get started.
        </p>
        <Button
          intent="outline"
          onPress={handleNewSession}
          isDisabled={creating}
        >
          <SquaresPlusIcon className="size-[18px] shrink-0" />
          {creating ? "Creating..." : "New Session"}
        </Button>
        <div className="text-sm text-muted-fg mt-4">
          Press{" "}
          <Keyboard className="inline-flex px-1.5 py-0.5 rounded bg-secondary text-secondary-fg text-xs font-mono">
            Shift + Enter
          </Keyboard>{" "}
          to start a new session
        </div>
      </div>
    </div>
  );
}

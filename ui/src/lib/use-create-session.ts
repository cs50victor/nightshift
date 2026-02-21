"use client";
import { useRouter } from "next/navigation";
import { useCallback, useState } from "react";
import { toast } from "@/components/ui/toast";
import { useSessionStore } from "@/stores/session-store";

export function useCreateSession() {
  const [creating, setCreating] = useState(false);
  const router = useRouter();
  const createSession = useSessionStore((s) => s.createSession);

  const handleNewSession = useCallback(async () => {
    if (creating) return;
    setCreating(true);
    try {
      const session = await createSession();
      toast.success("Session created");
      router.push(`/session/${session.id}`);
    } catch (err) {
      console.error("Failed to create session:", err);
      toast.error("Failed to create session");
    } finally {
      setCreating(false);
    }
  }, [creating, createSession, router]);

  return { creating, handleNewSession };
}

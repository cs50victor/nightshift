"use client";
import type { Agent } from "@opencode-ai/sdk";
import { useEffect } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectLabel,
  SelectTrigger,
} from "@/components/ui/select";
import { useAgentStore } from "@/stores/agent-store";
import { useConfigStore } from "@/stores/config-store";

interface AgentSelectProps {
  sessionId: string | null;
}

function isValidAgent(agents: Agent[], name?: string) {
  if (!name) return false;
  return agents.some((agent) => agent.name === name);
}

function getDefaultAgentName(agents: Agent[]) {
  return (
    agents.find((agent) => agent.name === "build")?.name ?? agents[0]?.name
  );
}

export function AgentSelect({ sessionId }: AgentSelectProps) {
  const agents = useConfigStore((s) => s.agents);

  const selectedAgent = useAgentStore((s) => s.getSelectedAgent(sessionId));
  const setSelectedAgent = useAgentStore((s) => s.setSelectedAgent);

  useEffect(() => {
    if (!sessionId || agents.length === 0) return;
    if (isValidAgent(agents, selectedAgent)) return;

    const fallback = getDefaultAgentName(agents);
    if (fallback) {
      setSelectedAgent(sessionId, fallback);
    }
  }, [agents, sessionId, selectedAgent, setSelectedAgent]);

  return (
    <Select
      aria-label="Agent"
      placeholder={agents.length === 0 ? "Loading agents..." : "Select agent"}
      className="w-auto"
      selectedKey={selectedAgent}
      onSelectionChange={(key) => {
        if (sessionId && key) {
          setSelectedAgent(sessionId, String(key));
        }
      }}
    >
      <SelectTrigger className="w-40" />
      <SelectContent items={agents}>
        {(agent) => (
          <SelectItem id={agent.name} textValue={agent.name}>
            <SelectLabel>{agent.name}</SelectLabel>
            {agent.description && (
              <span
                className="col-start-2 row-start-2 truncate max-w-[200px] text-muted-fg text-xs"
                title={agent.description}
              >
                {agent.description}
              </span>
            )}
          </SelectItem>
        )}
      </SelectContent>
    </Select>
  );
}

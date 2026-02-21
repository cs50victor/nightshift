"use client";

import { useCallback, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { QuestionRequest } from "@/stores/question-store";
import { useQuestionStore } from "@/stores/question-store";

interface QuestionDialogProps {
  sessionID: string;
}

export function QuestionDialog({ sessionID }: QuestionDialogProps) {
  const pending = useQuestionStore((s) => s.pending);
  const replyToQuestion = useQuestionStore((s) => s.reply);
  const rejectQuestion = useQuestionStore((s) => s.reject);
  const requests = useMemo(
    () => Object.values(pending).filter((q) => q.sessionID === sessionID),
    [pending, sessionID],
  );

  if (requests.length === 0) return null;

  return (
    <div className="sticky bottom-0 z-10 flex flex-col gap-3 border-t border-border bg-bg p-3">
      {requests.map((request) => (
        <QuestionRequestCard
          key={request.id}
          request={request}
          onReply={(answers) => replyToQuestion(request.id, answers)}
          onReject={() => rejectQuestion(request.id)}
        />
      ))}
    </div>
  );
}

interface QuestionRequestCardProps {
  request: QuestionRequest;
  onReply: (answers: string[][]) => void;
  onReject: () => void;
}

function QuestionRequestCard({
  request,
  onReply,
  onReject,
}: QuestionRequestCardProps) {
  const [answers, setAnswers] = useState<string[][]>(
    request.questions.map(() => []),
  );
  const [customInputs, setCustomInputs] = useState<string[]>(
    request.questions.map(() => ""),
  );

  const toggleOption = useCallback(
    (qIdx: number, label: string, multiple: boolean) => {
      setAnswers((prev) => {
        const next = [...prev];
        const current = next[qIdx] ?? [];
        if (multiple) {
          next[qIdx] = current.includes(label)
            ? current.filter((l) => l !== label)
            : [...current, label];
        } else {
          next[qIdx] = current.includes(label) ? [] : [label];
        }
        return next;
      });
    },
    [],
  );

  const setCustomInput = useCallback((qIdx: number, value: string) => {
    setCustomInputs((prev) => {
      const next = [...prev];
      next[qIdx] = value;
      return next;
    });
  }, []);

  const handleSubmit = useCallback(() => {
    const finalAnswers = answers.map((a, i) => {
      const custom = customInputs[i]?.trim();
      return custom ? [...a, custom] : a;
    });
    onReply(finalAnswers);
  }, [answers, customInputs, onReply]);

  return (
    <div className="rounded-lg border border-border bg-secondary/50 p-4">
      <div className="flex flex-col gap-4">
        {request.questions.map((q, qIdx) => (
          <div key={q.header} className="flex flex-col gap-2">
            <span className="text-xs font-medium text-muted-fg">
              {q.header}
            </span>
            <p className="text-sm text-fg">{q.question}</p>
            <div className="flex flex-wrap gap-2">
              {q.options.map((opt) => {
                const selected = (answers[qIdx] ?? []).includes(opt.label);
                return (
                  <Button
                    key={opt.label}
                    intent={selected ? "primary" : "outline"}
                    size="sm"
                    onPress={() =>
                      toggleOption(qIdx, opt.label, q.multiple === true)
                    }
                  >
                    {opt.label}
                  </Button>
                );
              })}
            </div>
            {q.custom !== false && (
              <Input
                placeholder="Custom answer..."
                value={customInputs[qIdx]}
                onChange={(e) => setCustomInput(qIdx, e.target.value)}
                className="mt-1"
              />
            )}
          </div>
        ))}
      </div>
      <div className="mt-4 flex items-center gap-2">
        <Button intent="primary" size="sm" onPress={handleSubmit}>
          Submit
        </Button>
        <Button intent="outline" size="sm" onPress={onReject}>
          Dismiss
        </Button>
      </div>
    </div>
  );
}

"use client";

import {
  ArrowUp,
  FileAudio,
  FileIcon,
  FileText,
  Film,
  ImageIcon,
  Plus,
  X,
} from "lucide-react";
import Image from "next/image";
import { useRef, useState } from "react";
import { AgentSelect } from "@/components/agent-select";
import {
  FileMentionPopover,
  useFileMention,
} from "@/components/file-mention-popover";
import { ModelSelect } from "@/components/model-select";
import { Button, buttonStyles } from "@/components/ui/button";
import { Menu, MenuContent, MenuItem, MenuTrigger } from "@/components/ui/menu";
import { Textarea } from "@/components/ui/textarea";

const MEDIA_PREFIXES = ["image/", "video/", "audio/", "application/pdf"];
function isMediaFile(mime: string): boolean {
  return MEDIA_PREFIXES.some((p) => mime.startsWith(p));
}

function attachmentIcon(mime: string) {
  if (mime.startsWith("video/"))
    return <Film className="size-6 text-muted-fg" />;
  if (mime.startsWith("audio/"))
    return <FileAudio className="size-6 text-muted-fg" />;
  if (mime === "application/pdf")
    return <FileIcon className="size-6 text-muted-fg" />;
  return null;
}

export interface Attachment {
  dataUrl: string;
  filename: string;
  mime: string;
}

interface ChatInputProps {
  sessionId: string;
  input: string;
  onInputChange: (value: string) => void;
  onSubmit: (e: React.FormEvent, attachments?: Attachment[]) => void;
  sending: boolean;
}

const TRIGGER_CLASS =
  "w-auto border-transparent bg-transparent px-2 py-1 text-xs hover:bg-muted rounded-lg shadow-none";

export function ChatInput({
  sessionId,
  input,
  onInputChange,
  onSubmit,
  sending,
}: ChatInputProps) {
  const fileMention = useFileMention();
  const [fileResults, setFileResults] = useState<string[]>([]);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const mediaInputRef = useRef<HTMLInputElement>(null);

  const readFileAsDataUrl = (file: File) => {
    const reader = new FileReader();
    reader.onload = () => {
      setAttachments((prev) => [
        ...prev,
        {
          dataUrl: reader.result as string,
          filename: file.name || "pasted-file",
          mime: file.type || "application/octet-stream",
        },
      ]);
    };
    reader.readAsDataURL(file);
  };

  const handleFileUpload = (
    e: React.ChangeEvent<HTMLInputElement>,
    type: "file" | "media",
  ) => {
    const files = e.target.files;
    if (!files?.length) return;
    if (type === "media") {
      for (const file of Array.from(files)) {
        readFileAsDataUrl(file);
      }
    } else {
      for (const file of Array.from(files)) {
        if (isMediaFile(file.type)) {
          readFileAsDataUrl(file);
        } else {
          const newValue = `${input}${input.endsWith(" ") ? "" : " "}@${file.name} `;
          onInputChange(newValue);
        }
      }
    }
    e.target.value = "";
  };

  const handlePaste = (e: React.ClipboardEvent) => {
    const items = e.clipboardData?.items;
    if (!items) return;
    for (const item of Array.from(items)) {
      if (isMediaFile(item.type)) {
        e.preventDefault();
        const file = item.getAsFile();
        if (file) readFileAsDataUrl(file);
        return;
      }
    }
  };

  const removeAttachment = (index: number) => {
    setAttachments((prev) => prev.filter((_, i) => i !== index));
  };

  return (
    <div className="shrink-0 p-4 relative">
      <FileMentionPopover
        isOpen={fileMention.isOpen}
        searchQuery={fileMention.searchQuery}
        textareaRef={textareaRef}
        mentionStart={fileMention.mentionStart}
        selectedIndex={fileMention.selectedIndex}
        onSelectedIndexChange={fileMention.setSelectedIndex}
        onFilesChange={setFileResults}
        onClose={fileMention.close}
        onSelect={(filePath) => {
          const newValue = fileMention.handleSelect(filePath, input);
          onInputChange(newValue);
        }}
      />

      <input
        ref={fileInputRef}
        type="file"
        multiple
        className="hidden"
        onChange={(e) => handleFileUpload(e, "file")}
      />
      <input
        ref={mediaInputRef}
        type="file"
        multiple
        accept="image/*,video/*,audio/*,application/pdf"
        className="hidden"
        onChange={(e) => handleFileUpload(e, "media")}
      />

      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (!input.trim() && attachments.length === 0) return;
          const submitted =
            attachments.length > 0 ? [...attachments] : undefined;
          setAttachments([]);
          onSubmit(e, submitted);
        }}
      >
        <div className="rounded-2xl border border-border bg-muted/40 overflow-hidden">
          <div className="p-3">
            <Textarea
              ref={textareaRef}
              value={input}
              onChange={(e) => {
                const value = e.target.value;
                onInputChange(value);
                if (fileMention.isOpen || value.includes("@")) {
                  const cursorPos = e.target.selectionStart ?? value.length;
                  fileMention.handleInputChange(value, cursorPos);
                }
              }}
              onInput={(e) => {
                const target = e.target as HTMLTextAreaElement;
                const value = target.value;
                if (value.includes("@")) {
                  const cursorPos = target.selectionStart ?? value.length;
                  fileMention.handleInputChange(value, cursorPos);
                }
              }}
              onSelect={(e) => {
                const target = e.target as HTMLTextAreaElement;
                if (fileMention.isOpen || input.includes("@")) {
                  const cursorPos = target.selectionStart ?? input.length;
                  fileMention.handleInputChange(input, cursorPos);
                }
              }}
              onPaste={handlePaste}
              onKeyDown={(e) => {
                const handled = fileMention.handleKeyDown(
                  e,
                  fileResults.length,
                );
                if (handled) {
                  if (
                    (e.key === "Enter" || e.key === "Tab") &&
                    fileResults.length > 0
                  ) {
                    const selectedFile = fileResults[fileMention.selectedIndex];
                    if (selectedFile) {
                      const newValue = fileMention.handleSelect(
                        selectedFile,
                        input,
                      );
                      onInputChange(newValue);
                    }
                  }
                  return;
                }
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  if (input.trim() || attachments.length > 0) {
                    const submitted =
                      attachments.length > 0 ? [...attachments] : undefined;
                    setAttachments([]);
                    onSubmit(e as unknown as React.FormEvent, submitted);
                  }
                }
              }}
              placeholder="Type your message... (use @ to mention files)"
              className="w-full resize-none min-h-24 max-h-40 overflow-y-auto border-0 bg-transparent p-0 focus:ring-0 focus:border-transparent shadow-none"
              rows={3}
            />
          </div>

          {attachments.length > 0 && (
            <div className="px-3 pb-2 flex flex-wrap gap-2">
              {attachments.map((att, i) => (
                <div
                  key={`${att.filename}-${i}`}
                  className="relative inline-block"
                >
                  {att.mime.startsWith("image/") ? (
                    <Image
                      src={att.dataUrl}
                      alt={att.filename}
                      width={64}
                      height={64}
                      className="h-16 w-16 rounded-lg object-cover border border-border"
                    />
                  ) : (
                    <div className="h-16 w-16 rounded-lg border border-border flex flex-col items-center justify-center bg-muted gap-1">
                      {attachmentIcon(att.mime)}
                      <span className="text-[9px] text-muted-fg truncate max-w-14 px-0.5">
                        {att.filename.split(".").pop()}
                      </span>
                    </div>
                  )}
                  <button
                    type="button"
                    onClick={() => removeAttachment(i)}
                    className="absolute -top-1.5 -right-1.5 rounded-full bg-fg text-bg p-0.5 cursor-pointer"
                  >
                    <X className="size-3" />
                  </button>
                </div>
              ))}
            </div>
          )}

          <div className="border-t border-border" />

          <div className="flex items-center gap-1 px-2 py-1.5">
            <Menu>
              <MenuTrigger
                aria-label="Attach"
                className={buttonStyles({
                  intent: "plain",
                  size: "sq-xs",
                  className: "rounded-lg cursor-default",
                })}
              >
                <Plus data-slot="icon" className="size-4" />
              </MenuTrigger>
              <MenuContent placement="top">
                <MenuItem onAction={() => mediaInputRef.current?.click()}>
                  <ImageIcon data-slot="icon" className="size-4" />
                  Upload Media
                </MenuItem>
                <MenuItem onAction={() => fileInputRef.current?.click()}>
                  <FileText data-slot="icon" className="size-4" />
                  Upload File
                </MenuItem>
              </MenuContent>
            </Menu>

            <AgentSelect
              sessionId={sessionId}
              triggerClassName={TRIGGER_CLASS}
            />
            <ModelSelect triggerClassName={TRIGGER_CLASS} />

            <div className="flex-1" />

            <Button
              type="submit"
              intent="secondary"
              size="sq-sm"
              isDisabled={
                (!input.trim() && attachments.length === 0) || sending
              }
              className="rounded-full"
            >
              <ArrowUp data-slot="icon" className="size-4" />
            </Button>
          </div>
        </div>
      </form>
    </div>
  );
}

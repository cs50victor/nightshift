"use client";

import { ArrowUp, FileText, ImageIcon, Plus, X } from "lucide-react";
import { useRef, useState } from "react";
import { AgentSelect } from "@/components/agent-select";
import {
  FileMentionPopover,
  useFileMention,
} from "@/components/file-mention-popover";
import { ModelSelect } from "@/components/model-select";
import { Button } from "@/components/ui/button";
import { Menu, MenuContent, MenuItem, MenuTrigger } from "@/components/ui/menu";
import { Textarea } from "@/components/ui/textarea";

interface ChatInputProps {
  sessionId: string;
  input: string;
  onInputChange: (value: string) => void;
  onSubmit: (e: React.FormEvent) => void;
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

  const [uploadedImage, setUploadedImage] = useState<string | null>(null);
  const [uploadedFileName, setUploadedFileName] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const imageInputRef = useRef<HTMLInputElement>(null);

  const handleFileUpload = (
    e: React.ChangeEvent<HTMLInputElement>,
    type: "file" | "image",
  ) => {
    const file = e.target.files?.[0];
    if (!file) return;
    if (type === "image") {
      setUploadedImage(URL.createObjectURL(file));
      setUploadedFileName(file.name);
    } else {
      const newValue =
        input + (input.endsWith(" ") ? "" : " ") + `@${file.name} `;
      onInputChange(newValue);
    }
    e.target.value = "";
  };

  const handleImageRemove = () => {
    setUploadedImage(null);
    setUploadedFileName(null);
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
        className="hidden"
        onChange={(e) => handleFileUpload(e, "file")}
      />
      <input
        ref={imageInputRef}
        type="file"
        accept="image/*"
        className="hidden"
        onChange={(e) => handleFileUpload(e, "image")}
      />

      <form onSubmit={onSubmit}>
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
                  if (input.trim()) {
                    onSubmit(e as unknown as React.FormEvent);
                  }
                }
              }}
              placeholder="Type your message... (use @ to mention files)"
              className="w-full resize-none min-h-24 max-h-40 overflow-y-auto border-0 bg-transparent p-0 focus:ring-0 focus:border-transparent shadow-none"
              rows={3}
            />
          </div>

          {uploadedImage && (
            <div className="px-3 pb-2">
              <div className="relative inline-block">
                {/* eslint-disable-next-line @next/next/no-img-element */}
                <img
                  src={uploadedImage}
                  alt={uploadedFileName ?? "uploaded"}
                  className="h-16 w-16 rounded-lg object-cover border border-border"
                />
                <button
                  type="button"
                  onClick={handleImageRemove}
                  className="absolute -top-1.5 -right-1.5 rounded-full bg-fg text-bg p-0.5 cursor-pointer"
                >
                  <X className="size-3" />
                </button>
              </div>
            </div>
          )}

          <div className="border-t border-border" />

          <div className="flex items-center gap-1 px-2 py-1.5">
            <Menu>
              <MenuTrigger>
                <Button
                  intent="plain"
                  size="sq-xs"
                  aria-label="Attach"
                  className="rounded-lg"
                >
                  <Plus data-slot="icon" className="size-4" />
                </Button>
              </MenuTrigger>
              <MenuContent placement="top">
                <MenuItem onAction={() => imageInputRef.current?.click()}>
                  <ImageIcon data-slot="icon" className="size-4" />
                  Upload Image
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
              isDisabled={(!input.trim() && !uploadedImage) || sending}
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

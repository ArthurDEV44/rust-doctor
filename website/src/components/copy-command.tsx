"use client";

import { Button } from "@/components/ui/button";
import { useCopyToClipboard } from "@/hooks/use-copy-to-clipboard";
import { CheckIcon, CopyIcon } from "lucide-react";

export function CopyCommand({ command }: { command: string }) {
  const { copyToClipboard, isCopied } = useCopyToClipboard();

  return (
    <Button
      variant="outline"
      className="justify-start gap-2 font-mono text-xs sm:text-sm w-full sm:w-auto min-w-0"
      onClick={() => copyToClipboard(command)}
    >
      <code className="truncate min-w-0">{command}</code>
      {isCopied ? (
        <CheckIcon className="size-3.5 shrink-0 text-success" />
      ) : (
        <CopyIcon className="size-3.5 shrink-0" />
      )}
    </Button>
  );
}

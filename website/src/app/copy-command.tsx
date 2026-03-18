"use client";

import { Button } from "@/components/ui/button";
import { useCopyToClipboard } from "@/hooks/use-copy-to-clipboard";
import { CheckIcon, CopyIcon } from "lucide-react";

export function CopyCommand({ command }: { command: string }) {
  const { copyToClipboard, isCopied } = useCopyToClipboard();

  return (
    <Button
      variant="outline"
      className="justify-start gap-2 font-mono text-sm w-full sm:w-auto"
      onClick={() => copyToClipboard(command)}
    >
      <code className="truncate">{command}</code>
      {isCopied ? (
        <CheckIcon className="size-3.5 text-success" />
      ) : (
        <CopyIcon className="size-3.5" />
      )}
    </Button>
  );
}

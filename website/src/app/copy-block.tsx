"use client";

import { Button } from "@/components/ui/button";
import { useCopyToClipboard } from "@/hooks/use-copy-to-clipboard";
import { CheckIcon, CopyIcon } from "lucide-react";

export function CopyBlock({
  label,
  command,
}: {
  label: string;
  command: string;
}) {
  const { copyToClipboard, isCopied } = useCopyToClipboard();

  return (
    <div>
      <p className="text-sm text-neutral-600 mb-1">{label}</p>
      <div className="relative group">
        <pre className="bg-neutral-900 border border-neutral-800 rounded-lg p-3 pr-10 text-sm text-neutral-300 overflow-x-auto font-mono">
          <code>{command}</code>
        </pre>
        <Button
          variant="ghost"
          size="icon-xs"
          className="absolute top-2.5 right-2.5 text-neutral-600 hover:text-white"
          onClick={() => copyToClipboard(command)}
        >
          {isCopied ? (
            <CheckIcon className="size-3.5 text-green-400" />
          ) : (
            <CopyIcon className="size-3.5" />
          )}
        </Button>
      </div>
    </div>
  );
}

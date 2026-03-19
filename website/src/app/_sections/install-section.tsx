import { CopyBlock } from "@/components/copy-block";

const INSTALL_METHODS = [
  { label: "npx", command: "npx -y rust-doctor@latest ." },
  { label: "cargo", command: "cargo install rust-doctor" },
  {
    label: "MCP",
    command:
      "claude mcp add --transport stdio -s user rust-doctor -- npx -y rust-doctor --mcp",
  },
  {
    label: "GitHub Actions",
    command: `- uses: ArthurDEV44/rust-doctor@v1\n  with:\n    token: \${{ secrets.GITHUB_TOKEN }}\n    fail-on: warning`,
  },
] as const;

export function InstallSection() {
  return (
    <section className="max-w-3xl mx-auto px-4 sm:px-6 py-16 sm:py-24 border-t border-border/30">
      <p className="text-xs uppercase tracking-[0.08em] text-muted-foreground mb-3">
        Install
      </p>
      <h2 className="text-2xl sm:text-3xl font-semibold tracking-[-0.03em] text-foreground mb-10">
        Pick your method.
      </h2>

      <div className="space-y-3">
        {INSTALL_METHODS.map((method) => (
          <CopyBlock
            key={method.label}
            label={method.label}
            command={method.command}
          />
        ))}
      </div>
    </section>
  );
}

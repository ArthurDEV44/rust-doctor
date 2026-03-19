import { CopyBlock } from "@/components/copy-block";

export function InstallSection() {
  return (
    <section className="mb-12">
      <h2 className="text-lg sm:text-xl md:text-2xl font-semibold mb-4 font-sans text-foreground">Installation</h2>
      <div className="space-y-3 text-sm">
        <CopyBlock label="npm / npx (no Rust toolchain required)" command="npx -y rust-doctor@latest ." />
        <CopyBlock label="cargo install" command="cargo install rust-doctor" />
        <CopyBlock label="Claude Code MCP" command="claude mcp add --transport stdio -s user rust-doctor -- npx -y rust-doctor --mcp" />
        <CopyBlock label="GitHub Actions" command={`- uses: ArthurDEV44/rust-doctor@v1\n  with:\n    token: \${{ secrets.GITHUB_TOKEN }}\n    fail-on: warning`} />
      </div>
    </section>
  );
}

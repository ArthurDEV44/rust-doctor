const MCP_TOOLS = [
  {
    name: "scan",
    description: "Full diagnostics + health score",
  },
  {
    name: "score",
    description: "Quick 0–100 pass/fail",
  },
  {
    name: "explain_rule",
    description: "Rule docs + fix guidance",
  },
  {
    name: "list_rules",
    description: "Browse all available checks",
  },
] as const;

export function McpSection() {
  return (
    <section className="max-w-3xl mx-auto px-4 sm:px-6 py-16 sm:py-24 border-t border-border/30">
      <p className="text-xs uppercase tracking-[0.08em] text-muted-foreground mb-3">
        MCP Server
      </p>
      <h2 className="text-2xl sm:text-3xl font-semibold tracking-[-0.03em] text-foreground mb-4">
        Built for AI coding assistants.
      </h2>
      <p className="text-sm text-muted-foreground leading-relaxed max-w-xl mb-10">
        rust-doctor includes a built-in{" "}
        <a
          href="https://modelcontextprotocol.io"
          target="_blank"
          rel="noopener noreferrer"
          className="text-foreground underline underline-offset-4 decoration-border hover:decoration-foreground transition-colors"
        >
          Model Context Protocol
        </a>{" "}
        server. Claude Code, Cursor, VS Code — any MCP-compatible tool can scan
        your project, explain rules, and suggest fixes.
      </p>

      <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
        {MCP_TOOLS.map((tool) => (
          <div
            key={tool.name}
            className="border border-border/50 rounded-md p-4"
          >
            <code className="text-sm text-foreground font-medium">
              {tool.name}
            </code>
            <p className="text-xs text-muted-foreground mt-2">
              {tool.description}
            </p>
          </div>
        ))}
      </div>
    </section>
  );
}

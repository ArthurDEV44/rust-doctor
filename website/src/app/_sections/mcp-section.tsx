import { Card, CardContent } from "@/components/ui/card";
import { Kbd } from "@/components/ui/kbd";

export function McpSection() {
  return (
    <section className="mb-12">
      <h2 className="text-lg sm:text-xl md:text-2xl font-semibold mb-4 font-sans text-foreground">
        MCP server for AI coding assistants
      </h2>
      <p className="text-muted-foreground mb-4">
        rust-doctor includes a built-in{" "}
        <a
          href="https://modelcontextprotocol.io"
          target="_blank"
          rel="noopener noreferrer"
          className="underline hover:text-foreground transition-colors"
        >
          Model Context Protocol
        </a>{" "}
        server. AI coding assistants can scan projects, explain rules, and
        suggest fixes directly. Works with Claude Code, Cursor, VS Code, and
        any MCP-compatible tool.
      </p>
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 text-sm">
        <Card>
          <CardContent className="p-3">
            <Kbd>scan</Kbd>
            <p className="text-muted-foreground mt-1">Full diagnostics + score</p>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-3">
            <Kbd>score</Kbd>
            <p className="text-muted-foreground mt-1">Quick 0&ndash;100 score</p>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-3">
            <Kbd>explain_rule</Kbd>
            <p className="text-muted-foreground mt-1">Rule docs + fix guidance</p>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-3">
            <Kbd>list_rules</Kbd>
            <p className="text-muted-foreground mt-1">All available rules</p>
          </CardContent>
        </Card>
      </div>
    </section>
  );
}

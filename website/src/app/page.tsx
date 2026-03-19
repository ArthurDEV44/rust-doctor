import { Terminal } from "./terminal";
import { CopyBlock } from "./copy-block";
import { ThemeToggle } from "./theme-toggle";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { Kbd } from "@/components/ui/kbd";
import {
  Accordion,
  AccordionItem,
  AccordionTrigger,
  AccordionPanel,
} from "@/components/ui/accordion";

const FAQ_ITEMS = [
  {
    question: "Does Rust need static analysis if the compiler already catches so much?",
    answer:
      "Yes. The Rust compiler catches memory safety and type errors, but it cannot detect logic issues like hardcoded secrets, performance anti-patterns (excessive cloning, blocking in async), architectural problems, or dependency vulnerabilities. rust-doctor fills this gap with 700+ clippy lints, 18 custom AST rules, CVE detection via cargo-audit, and unused dependency detection via cargo-machete.",
  },
  {
    question: "How do I measure Rust code quality?",
    answer:
      "rust-doctor provides a 0-100 health score for any Rust project. The score is calculated as 100 minus penalties for unique rule violations: 1.5 points per error-level rule, 0.75 per warning-level rule. Scores above 75 indicate a healthy codebase, 50-74 needs work, and below 50 is critical. Run 'npx -y rust-doctor@latest .' at your project root to get your score instantly.",
  },
  {
    question: "What is a Rust health score?",
    answer:
      "A Rust health score is a single 0-100 metric that summarizes the overall quality of a Rust codebase. rust-doctor calculates it by scanning for security vulnerabilities, performance issues, correctness bugs, architectural anti-patterns, and dependency problems. Unlike raw lint counts, the score counts unique rules violated. Fixing all instances of one issue removes the entire penalty.",
  },
  {
    question: "How does rust-doctor compare to Clippy?",
    answer:
      "rust-doctor runs Clippy internally (700+ lints with severity overrides) and adds 18 custom AST rules that Clippy doesn't cover: hardcoded secret detection, blocking-in-async detection, framework-specific rules for tokio/axum/actix-web, and architectural anti-patterns. It also integrates cargo-audit for CVE scanning and cargo-machete for unused dependency detection, all producing a single health score.",
  },
  {
    question: "Can I use rust-doctor with AI coding assistants?",
    answer:
      "Yes. rust-doctor includes a built-in MCP (Model Context Protocol) server. Add it to Claude Code with 'claude mcp add --transport stdio -s user rust-doctor -- npx -y rust-doctor --mcp'. It also works with Cursor, VS Code, and any MCP-compatible tool. AI assistants can scan your project, explain rules, and suggest fixes directly.",
  },
  {
    question: "How do I add rust-doctor to CI/CD?",
    answer:
      "Add the GitHub Action to your workflow: 'uses: ArthurDEV44/rust-doctor@v1' with 'fail-on: warning' to block PRs with issues. It posts a PR comment with the health score, error/warning counts, and top diagnostics. It also supports SARIF output for GitHub Code Scanning integration.",
  },
  {
    question: "How do I track technical debt in a Rust project?",
    answer:
      "rust-doctor quantifies technical debt as a health score: 100 means zero detected issues, lower scores indicate accumulated debt. Each rule category (security, performance, correctness, architecture, dependencies) contributes independently, so you can see exactly where debt is concentrated. Run it in CI to track score trends over time and prevent debt from growing.",
  },
  {
    question: "What issues does rust-doctor catch that other tools miss?",
    answer:
      "rust-doctor detects hardcoded secrets in connection strings, blocking I/O inside async functions, panic!() in library crates, excessive .clone() in hot loops, and framework-specific anti-patterns for tokio/axum/actix-web. These are logic and architecture issues that the Rust compiler, Clippy, and cargo-audit each miss individually. rust-doctor combines all three plus 18 custom AST rules into one scan.",
  },
];

const faqJsonLd = {
  "@context": "https://schema.org",
  "@type": "FAQPage",
  mainEntity: FAQ_ITEMS.map((item) => ({
    "@type": "Question",
    name: item.question,
    acceptedAnswer: {
      "@type": "Answer",
      text: item.answer,
    },
  })),
};

const howToJsonLd = {
  "@context": "https://schema.org",
  "@type": "HowTo",
  name: "How to scan a Rust project for code health issues",
  description:
    "Install and run rust-doctor to get a 0-100 health score for any Rust project. Detects security, performance, correctness, architecture, and dependency issues.",
  totalTime: "PT2M",
  step: [
    {
      "@type": "HowToStep",
      position: 1,
      name: "Run rust-doctor via npx",
      text: "Open a terminal at your Rust project root and run: npx -y rust-doctor@latest . — No Rust toolchain required, the npm package bundles the binary.",
    },
    {
      "@type": "HowToStep",
      position: 2,
      name: "Review the health score",
      text: "rust-doctor outputs a 0-100 health score with diagnostic details. Scores 75-100 are Great, 50-74 Need work, and 0-49 are Critical.",
    },
    {
      "@type": "HowToStep",
      position: 3,
      name: "Add as MCP server (optional)",
      text: "For AI-assisted fixes, add rust-doctor as an MCP server: claude mcp add --transport stdio -s user rust-doctor -- npx -y rust-doctor --mcp",
    },
    {
      "@type": "HowToStep",
      position: 4,
      name: "Add to CI/CD (optional)",
      text: "Add the GitHub Action to your workflow: uses: ArthurDEV44/rust-doctor@v1 with fail-on: warning to block PRs with code health issues.",
    },
  ],
};

const breadcrumbJsonLd = {
  "@context": "https://schema.org",
  "@type": "BreadcrumbList",
  itemListElement: [
    {
      "@type": "ListItem",
      position: 1,
      name: "Home",
      item: "https://rust-doctor.dev",
    },
  ],
};

export default function Home() {
  return (
    <>
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{
          __html: JSON.stringify(faqJsonLd).replace(/</g, "\\u003c"),
        }}
      />
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{
          __html: JSON.stringify(howToJsonLd).replace(/</g, "\\u003c"),
        }}
      />
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{
          __html: JSON.stringify(breadcrumbJsonLd).replace(/</g, "\\u003c"),
        }}
      />

      {/* Hero — animated terminal demo (full viewport) */}
      <div className="min-h-[100svh] bg-background flex justify-center items-start p-3 sm:p-4 md:p-8 pt-8 sm:pt-12 md:pt-20">
        <Terminal />
      </div>

      {/* Server-rendered SEO content */}
      <main className="max-w-3xl mx-auto px-4 sm:px-6 py-12 sm:py-16 md:py-24 font-mono text-[14px] md:text-[15px] min-w-0 w-full overflow-hidden">
        <h1 className="text-2xl sm:text-3xl md:text-4xl lg:text-5xl font-bold tracking-tight mb-4 font-sans text-foreground">
          rust-doctor: Rust code health scanner
        </h1>
        <p className="text-sm sm:text-base text-muted-foreground mb-8 sm:mb-12 max-w-2xl leading-relaxed">
          A unified code health tool for Rust. Scans for security, performance,
          correctness, architecture, and dependency issues, then outputs a
          0&ndash;100 health score with actionable diagnostics.
        </p>

        {/* What it checks */}
        <section className="mb-12">
          <h2 className="text-lg sm:text-xl md:text-2xl font-semibold mb-4 font-sans text-foreground">
            What does rust-doctor check?
          </h2>
          <ul className="space-y-2 text-muted-foreground">
            <li>
              <strong className="text-foreground">700+ clippy lints</strong>{" "}
              with severity overrides across pedantic, nursery, and cargo groups
            </li>
            <li>
              <strong className="text-foreground">18 custom AST rules</strong>{" "}
              via syn: error handling, performance, security, async, and
              framework anti-patterns
            </li>
            <li>
              <strong className="text-foreground">CVE detection</strong> via
              cargo-audit, scanning dependencies against the RustSec Advisory
              Database
            </li>
            <li>
              <strong className="text-foreground">
                Unused dependency detection
              </strong>{" "}
              via cargo-machete. Finds deps in Cargo.toml that your code never
              imports
            </li>
            <li>
              <strong className="text-foreground">
                Framework-specific rules
              </strong>{" "}
              for tokio, axum, and actix-web: missing async handlers, blocking
              in async, spawn without move
            </li>
          </ul>
        </section>

        {/* Health score */}
        <section className="mb-12">
          <h2 className="text-lg sm:text-xl md:text-2xl font-semibold mb-4 font-sans text-foreground">
            How is the health score calculated?
          </h2>
          <p className="text-muted-foreground mb-4">
            Score = 100 &minus; (unique error rules &times; 1.5) &minus; (unique
            warning rules &times; 0.75), clamped to 0&ndash;100. The score
            counts unique rules violated, not total occurrences. Fixing all
            instances of one issue removes the entire penalty.
          </p>
          <div className="grid grid-cols-3 gap-2 sm:gap-4 text-center text-xs sm:text-sm">
            <Card>
              <CardContent className="p-3">
                <Badge variant="success" size="lg">75&ndash;100</Badge>
                <div className="text-muted-foreground mt-1">Great</div>
              </CardContent>
            </Card>
            <Card>
              <CardContent className="p-3">
                <Badge variant="warning" size="lg">50&ndash;74</Badge>
                <div className="text-muted-foreground mt-1">Needs work</div>
              </CardContent>
            </Card>
            <Card>
              <CardContent className="p-3">
                <Badge variant="error" size="lg">0&ndash;49</Badge>
                <div className="text-muted-foreground mt-1">Critical</div>
              </CardContent>
            </Card>
          </div>
        </section>

        {/* MCP Server */}
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

        {/* Installation */}
        <section className="mb-12">
          <h2 className="text-lg sm:text-xl md:text-2xl font-semibold mb-4 font-sans text-foreground">Installation</h2>
          <div className="space-y-3 text-sm">
            <CopyBlock label="npm / npx (no Rust toolchain required)" command="npx -y rust-doctor@latest ." />
            <CopyBlock label="cargo install" command="cargo install rust-doctor" />
            <CopyBlock label="Claude Code MCP" command="claude mcp add --transport stdio -s user rust-doctor -- npx -y rust-doctor --mcp" />
            <CopyBlock label="GitHub Actions" command={`- uses: ArthurDEV44/rust-doctor@v1\n  with:\n    token: \${{ secrets.GITHUB_TOKEN }}\n    fail-on: warning`} />
          </div>
        </section>

        {/* Custom rules */}
        <section className="mb-12">
          <h2 className="text-lg sm:text-xl md:text-2xl font-semibold mb-4 font-sans text-foreground">
            18 custom AST rules
          </h2>

          {/* Mobile: card layout */}
          <div className="space-y-2 sm:hidden">
            {[
              ["Error Handling", "unwrap-in-production", "Warning"],
              ["Error Handling", "panic-in-library", "Error"],
              ["Error Handling", "box-dyn-error-in-public-api", "Warning"],
              ["Error Handling", "result-unit-error", "Warning"],
              ["Performance", "excessive-clone", "Warning"],
              ["Performance", "string-from-literal", "Warning"],
              ["Performance", "collect-then-iterate", "Warning"],
              ["Performance", "large-enum-variant", "Warning"],
              ["Performance", "unnecessary-allocation", "Warning"],
              ["Security", "hardcoded-secrets", "Error"],
              ["Security", "unsafe-block-audit", "Warning"],
              ["Security", "sql-injection-risk", "Error"],
              ["Async", "blocking-in-async", "Warning"],
              ["Async", "block-on-in-async", "Error"],
              ["Framework", "tokio-main-missing", "Error"],
              ["Framework", "tokio-spawn-without-move", "Warning"],
              ["Framework", "axum-handler-not-async", "Warning"],
              ["Framework", "actix-blocking-handler", "Error"],
            ].map(([cat, rule, sev]) => (
              <div key={rule} className="flex items-center justify-between gap-2 border border-border rounded-lg p-3">
                <div className="min-w-0">
                  <span className="text-muted-foreground text-xs">{cat}</span>
                  <div className="truncate"><Kbd>{rule}</Kbd></div>
                </div>
                <Badge variant={sev === "Error" ? "error" : "warning"} size="sm" className="shrink-0">
                  {sev}
                </Badge>
              </div>
            ))}
          </div>

          {/* Desktop: table layout */}
          <div className="hidden sm:block overflow-x-auto">
            <table className="w-full text-sm text-left">
              <thead>
                <tr className="border-b border-border text-muted-foreground">
                  <th className="py-2 pr-4">Category</th>
                  <th className="py-2 pr-4">Rule</th>
                  <th className="py-2">Severity</th>
                </tr>
              </thead>
              <tbody className="text-muted-foreground">
                {[
                  ["Error Handling", "unwrap-in-production", "Warning"],
                  ["Error Handling", "panic-in-library", "Error"],
                  ["Error Handling", "box-dyn-error-in-public-api", "Warning"],
                  ["Error Handling", "result-unit-error", "Warning"],
                  ["Performance", "excessive-clone", "Warning"],
                  ["Performance", "string-from-literal", "Warning"],
                  ["Performance", "collect-then-iterate", "Warning"],
                  ["Performance", "large-enum-variant", "Warning"],
                  ["Performance", "unnecessary-allocation", "Warning"],
                  ["Security", "hardcoded-secrets", "Error"],
                  ["Security", "unsafe-block-audit", "Warning"],
                  ["Security", "sql-injection-risk", "Error"],
                  ["Async", "blocking-in-async", "Warning"],
                  ["Async", "block-on-in-async", "Error"],
                  ["Framework", "tokio-main-missing", "Error"],
                  ["Framework", "tokio-spawn-without-move", "Warning"],
                  ["Framework", "axum-handler-not-async", "Warning"],
                  ["Framework", "actix-blocking-handler", "Error"],
                ].map(([cat, rule, sev]) => (
                  <tr key={rule} className="border-b border-border">
                    <td className="py-2 pr-4 text-muted-foreground">{cat}</td>
                    <td className="py-2 pr-4">
                      <Kbd>{rule}</Kbd>
                    </td>
                    <td className="py-2">
                      <Badge variant={sev === "Error" ? "error" : "warning"} size="sm">
                        {sev}
                      </Badge>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>

        {/* FAQ */}
        <section className="mb-12">
          <h2 className="text-lg sm:text-xl md:text-2xl font-semibold mb-6 font-sans text-foreground">
            Frequently asked questions
          </h2>
          <Accordion>
            {FAQ_ITEMS.map((item) => (
              <AccordionItem key={item.question} value={item.question}>
                <AccordionTrigger>{item.question}</AccordionTrigger>
                <AccordionPanel>
                  <p className="text-muted-foreground text-sm leading-relaxed">
                    {item.answer}
                  </p>
                </AccordionPanel>
              </AccordionItem>
            ))}
          </Accordion>
        </section>

        {/* Footer */}
        <footer className="border-t border-border pt-8 text-sm text-muted-foreground space-y-3">
          <div className="flex flex-col sm:flex-row justify-between gap-2">
            <span>MIT OR Apache-2.0</span>
            <div className="flex gap-4">
              <a
                href="https://github.com/ArthurDEV44/rust-doctor"
                target="_blank"
                rel="noopener noreferrer"
                className="hover:text-foreground transition-colors"
              >
                GitHub
              </a>
              <a
                href="https://crates.io/crates/rust-doctor"
                target="_blank"
                rel="noopener noreferrer"
                className="hover:text-foreground transition-colors"
              >
                crates.io
              </a>
              <a
                href="https://www.npmjs.com/package/rust-doctor"
                target="_blank"
                rel="noopener noreferrer"
                className="hover:text-foreground transition-colors"
              >
                npm
              </a>
            </div>
          </div>
          <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-2">
            <p>
              Developed by{" "}
              <a
                href="https://arthurjean.com/"
                target="_blank"
                rel="noopener noreferrer"
                className="text-foreground/70 hover:text-foreground transition-colors"
              >
                Arthur Jean
              </a>
              {" "}at{" "}
              <a
                href="https://strivex.fr/"
                target="_blank"
                rel="noopener noreferrer"
                className="text-foreground/70 hover:text-foreground transition-colors"
              >
                StriveX
              </a>
            </p>
            <ThemeToggle />
          </div>
        </footer>
      </main>
    </>
  );
}

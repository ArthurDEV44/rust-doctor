export const FAQ_ITEMS = [
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

export const AST_RULES = [
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
] as const;

export const faqJsonLd = {
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

export const howToJsonLd = {
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

export const breadcrumbJsonLd = {
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

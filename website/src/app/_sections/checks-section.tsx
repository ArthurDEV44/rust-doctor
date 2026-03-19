const CHECKS = [
  {
    label: "LINTS",
    stat: "700+",
    title: "Clippy lints",
    description:
      "Severity overrides across pedantic, nursery, and cargo groups.",
  },
  {
    label: "AST",
    stat: "18",
    title: "Custom rules",
    description:
      "Error handling, performance, security, async, and framework anti-patterns via syn.",
  },
  {
    label: "DEPS",
    stat: "5",
    title: "Cargo tools",
    description:
      "cargo-audit, cargo-deny, cargo-geiger, cargo-machete, cargo-semver-checks.",
  },
  {
    label: "FRAMEWORKS",
    stat: "3",
    title: "Runtime targets",
    description:
      "tokio, axum, actix-web — blocking in async, missing handlers, spawn without move.",
  },
] as const;

export function ChecksSection() {
  return (
    <section className="max-w-3xl mx-auto px-4 sm:px-6 py-16 sm:py-24">
      <p className="text-xs uppercase tracking-[0.08em] text-muted-foreground mb-3">
        What it checks
      </p>
      <h2 className="text-2xl sm:text-3xl font-semibold tracking-[-0.03em] text-foreground mb-10">
        Everything clippy doesn&apos;t.
      </h2>

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        {CHECKS.map((check) => (
          <div
            key={check.label}
            className="border border-border/50 rounded-md p-5"
          >
            <p className="text-xs uppercase tracking-[0.08em] text-muted-foreground/60 mb-3">
              {check.label}
            </p>
            <p className="text-3xl font-bold text-foreground leading-none mb-1">
              {check.stat}
            </p>
            <p className="text-sm text-foreground font-medium">
              {check.title}
            </p>
            <p className="text-sm text-muted-foreground mt-2 leading-relaxed">
              {check.description}
            </p>
          </div>
        ))}
      </div>

      <p className="mt-6 text-xs text-muted-foreground">
        External tools are optional — missing ones are skipped gracefully. Run{" "}
        <code className="text-foreground">rust-doctor --install-deps</code> to
        install them all at once.
      </p>
    </section>
  );
}

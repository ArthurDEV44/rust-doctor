export function ChecksSection() {
  return (
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
          <strong className="text-foreground">Dependency auditing</strong> —
          CVE scanning (cargo-audit), supply-chain checks (cargo-deny), unused
          deps (cargo-machete), unsafe code budget (cargo-geiger), semver
          violations (cargo-semver-checks)
        </li>
        <li>
          <strong className="text-foreground">
            Framework-specific rules
          </strong>{" "}
          for tokio, axum, and actix-web: missing async handlers, blocking
          in async, spawn without move
        </li>
      </ul>
      <p className="mt-3 text-xs text-muted-foreground">
        External tools are optional — missing ones are skipped gracefully.
        Run{" "}
        <code className="text-foreground">rust-doctor --install-deps</code>{" "}
        to install them all at once.
      </p>
    </section>
  );
}

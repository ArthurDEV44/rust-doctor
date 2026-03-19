import { ThemeToggle } from "@/components/theme-toggle";

export function SiteFooter() {
  return (
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
  );
}

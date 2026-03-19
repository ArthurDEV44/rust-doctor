import Link from "next/link";
import { CopyCommand } from "@/components/copy-command";

export function SiteFooter() {
  return (
    <footer className="max-w-3xl mx-auto px-4 sm:px-6 pt-16 sm:pt-24 pb-12 border-t border-border/30">
      {/* Final CTA */}
      <div className="text-center mb-16">
        <h2 className="text-2xl sm:text-3xl font-semibold tracking-[-0.03em] text-foreground mb-4">
          Try it now.
        </h2>
        <p className="text-sm text-muted-foreground mb-6">
          One command. See what clippy missed.
        </p>
        <div className="flex justify-center">
          <CopyCommand command="npx -y rust-doctor@latest ." />
        </div>
      </div>

      {/* Footer links */}
      <div className="flex flex-col sm:flex-row justify-between gap-4 text-xs text-muted-foreground/60">
        <div className="flex gap-4">
          <a
            href="https://github.com/ArthurDEV44/rust-doctor"
            target="_blank"
            rel="noopener noreferrer"
            className="text-foreground hover:text-foreground/80 transition-colors"
          >
            GitHub
          </a>
          <a
            href="https://crates.io/crates/rust-doctor"
            target="_blank"
            rel="noopener noreferrer"
            className="text-foreground hover:text-foreground/80 transition-colors"
          >
            crates.io
          </a>
          <a
            href="https://www.npmjs.com/package/rust-doctor"
            target="_blank"
            rel="noopener noreferrer"
            className="text-foreground hover:text-foreground/80 transition-colors"
          >
            npm
          </a>
          <Link
            href="/docs"
            className="text-foreground hover:text-foreground/80 transition-colors"
          >
            Docs
          </Link>
          <Link
            href="/blog"
            className="text-foreground hover:text-foreground/80 transition-colors"
          >
            Blog
          </Link>
        </div>
        <p>
          Built by{" "}
          <a
            href="https://arthurjean.com/"
            target="_blank"
            rel="noopener noreferrer"
            className="text-foreground hover:text-foreground/80 transition-colors"
          >
            Arthur Jean
          </a>
          {" "}at{" "}
          <a
            href="https://strivex.fr/"
            target="_blank"
            rel="noopener noreferrer"
            className="text-foreground hover:text-foreground/80 transition-colors"
          >
            StriveX
          </a>
        </p>
      </div>
    </footer>
  );
}

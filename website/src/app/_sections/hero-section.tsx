import { Terminal } from "@/components/terminal";
import { CopyCommand } from "@/components/copy-command";
import { Button } from "@/components/ui/button";
import { GithubIcon } from "lucide-react";

export function HeroSection() {
  return (
    <section className="relative overflow-hidden">
      {/* Gradient background — dark */}
      <div
        className="pointer-events-none absolute inset-0 hidden dark:block"
        aria-hidden="true"
        style={{
          background:
            "linear-gradient(145deg, #1a0e05 0%, #0d0d0d 40%, #0a0c12 70%, #0d0d0d 100%)",
        }}
      />
      {/* Gradient background — light */}
      <div
        className="pointer-events-none absolute inset-0 dark:hidden"
        aria-hidden="true"
        style={{
          background:
            "linear-gradient(145deg, #fdf4ec 0%, #ffffff 40%, #f0f2f8 70%, #ffffff 100%)",
        }}
      />

      <div className="relative max-w-4xl mx-auto px-4 sm:px-6 pt-16 sm:pt-24 md:pt-32 pb-12 sm:pb-16">
        {/* Tagline */}
        <div className="text-center mb-10 sm:mb-14">
          <h1 className="text-3xl sm:text-4xl md:text-5xl lg:text-6xl font-bold tracking-tight text-foreground font-mono">
            Know if your Rust code
            <br />
            is actually healthy.
          </h1>
          <p className="mt-4 sm:mt-6 text-base sm:text-lg text-muted-foreground max-w-xl mx-auto leading-relaxed">
            One command. One score. Security, performance, correctness,
            architecture, and dependencies. Scanned in seconds.
          </p>

          {/* CTA commands */}
          <div className="mt-8 flex flex-col sm:flex-row items-center justify-center gap-3">
            <CopyCommand command="npx -y rust-doctor@latest ." />
            <Button
              variant="default"
              render={
                <a
                  href="https://github.com/ArthurDEV44/rust-doctor"
                  target="_blank"
                  rel="noopener noreferrer"
                />
              }
            >
              <GithubIcon />
              Star on GitHub
            </Button>
          </div>
        </div>

        {/* Ghostty-style terminal */}
        <div className="relative mx-auto max-w-2xl">
          {/* Outer glow */}
          <div className="absolute -inset-px rounded-xl bg-gradient-to-b from-black/[0.04] to-transparent dark:from-white/[0.06] dark:to-transparent" />

          {/* Terminal chrome */}
          <div className="relative rounded-xl border border-black/[0.08] dark:border-white/[0.08] bg-white/70 dark:bg-black/50 backdrop-blur-xl shadow-2xl shadow-black/10 dark:shadow-black/50 overflow-hidden">
            {/* Title bar */}
            <div className="flex items-center gap-2 px-4 py-3 border-b border-black/[0.06] dark:border-white/[0.06]">
              <div className="flex gap-1.5">
                <div className="size-3 rounded-full bg-red-500/80" />
                <div className="size-3 rounded-full bg-yellow-500/80" />
                <div className="size-3 rounded-full bg-green-500/80" />
              </div>
              <span className="text-xs text-neutral-400 dark:text-neutral-500 ml-2 font-mono">
                rust-doctor
              </span>
            </div>

            {/* Terminal content */}
            <div className="p-4 sm:p-6 will-change-contents">
              <Terminal />
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

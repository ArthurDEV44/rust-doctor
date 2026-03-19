import { HeroSection } from "./_sections/hero-section";
import { ChecksSection } from "./_sections/checks-section";
import { ScoreSection } from "./_sections/score-section";
import { McpSection } from "./_sections/mcp-section";
import { InstallSection } from "./_sections/install-section";
import { RulesSection } from "./_sections/rules-section";
import { FaqSection } from "./_sections/faq-section";
import { SiteFooter } from "./_sections/site-footer";
import { faqJsonLd, howToJsonLd, breadcrumbJsonLd } from "@/lib/data";

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

      <HeroSection />

      <main className="max-w-3xl mx-auto px-4 sm:px-6 py-12 sm:py-16 md:py-24 font-mono text-[14px] md:text-[15px] min-w-0 w-full overflow-hidden">
        <h1 className="text-2xl sm:text-3xl md:text-4xl lg:text-5xl font-bold tracking-tight mb-4 font-sans text-foreground">
          rust-doctor: Rust code health scanner
        </h1>
        <p className="text-sm sm:text-base text-muted-foreground mb-8 sm:mb-12 max-w-2xl leading-relaxed">
          A unified code health tool for Rust. Scans for security, performance,
          correctness, architecture, and dependency issues, then outputs a
          0&ndash;100 health score with actionable diagnostics.
        </p>

        <ChecksSection />
        <ScoreSection />
        <McpSection />
        <InstallSection />
        <RulesSection />
        <FaqSection />
        <SiteFooter />
      </main>
    </>
  );
}

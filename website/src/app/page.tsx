import { SiteHeader } from "@/components/site-header";
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
      <SiteHeader />
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

      <main className="font-mono text-[14px] md:text-[15px] min-w-0 w-full overflow-hidden">
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

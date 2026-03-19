import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import { RootProvider } from "fumadocs-ui/provider/next";
import "./globals.css";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  metadataBase: new URL("https://rust-doctor.dev"),
  title: "rust-doctor | Rust code health scanner",
  description:
    "Scan Rust projects for security, performance, correctness, architecture, and dependency issues. Get a 0-100 health score with actionable diagnostics. MCP server for Claude Code, Cursor, and VS Code.",
  keywords: [
    "rust linter",
    "rust code quality",
    "rust static analysis",
    "rust security scanner",
    "rust clippy alternative",
    "rust diagnostics tool",
    "rust health score",
    "mcp server rust",
    "rust code review",
    "cargo audit",
  ],
  alternates: {
    canonical: "/",
  },
  robots: {
    index: true,
    follow: true,
    googleBot: {
      index: true,
      follow: true,
      "max-image-preview": "large",
      "max-snippet": -1,
    },
  },
  openGraph: {
    title: "rust-doctor | Rust code health scanner",
    description:
      "Scan Rust projects for security, performance, correctness, and architecture issues. 0-100 health score with actionable diagnostics.",
    url: "https://rust-doctor.dev",
    siteName: "rust-doctor",
    type: "website",
    locale: "en_US",
  },
  twitter: {
    card: "summary_large_image",
    title: "rust-doctor | Rust code health scanner",
    description:
      "Scan Rust projects for security, performance, correctness, and architecture issues. 0-100 health score with actionable diagnostics.",
  },
};

const organizationJsonLd = {
  "@context": "https://schema.org",
  "@type": "Organization",
  name: "StriveX",
  url: "https://strivex.fr",
  logo: {
    "@type": "ImageObject",
    url: "https://rust-doctor.dev/images/rusty-happy.png",
    width: 512,
    height: 512,
  },
  founder: {
    "@type": "Person",
    name: "Arthur Jean",
    url: "https://arthurjean.com",
    sameAs: ["https://github.com/ArthurDEV44"],
  },
  sameAs: [
    "https://github.com/ArthurDEV44",
    "https://strivex.fr",
  ],
};

const websiteJsonLd = {
  "@context": "https://schema.org",
  "@type": "WebSite",
  name: "rust-doctor",
  url: "https://rust-doctor.dev",
  description:
    "A unified code health tool for Rust. Scan, score, and fix your codebase.",
  publisher: { "@type": "Organization", name: "StriveX", url: "https://strivex.fr" },
};

const softwareJsonLd = {
  "@context": "https://schema.org",
  "@type": "SoftwareApplication",
  name: "rust-doctor",
  applicationCategory: "DeveloperApplication",
  applicationSubCategory: "Static Analysis Tool",
  operatingSystem: "Linux, macOS, Windows",
  url: "https://rust-doctor.dev",
  downloadUrl: "https://crates.io/crates/rust-doctor",
  installUrl: "https://www.npmjs.com/package/rust-doctor",
  softwareVersion: process.env.NEXT_PUBLIC_VERSION || "0.1.3",
  description:
    "Scan Rust projects for security, performance, correctness, architecture, and dependency issues. Get a 0-100 health score with actionable diagnostics.",
  featureList: [
    "700+ clippy lints with severity overrides",
    "18 custom AST rules via syn",
    "CVE detection via cargo-audit",
    "Unused dependency detection via cargo-machete",
    "Framework-specific rules for tokio, axum, actix-web",
    "Built-in MCP server for AI coding assistants",
    "GitHub Actions integration with PR comments",
    "SARIF output for GitHub Code Scanning",
  ],
  offers: {
    "@type": "Offer",
    price: "0",
    priceCurrency: "USD",
    availability: "https://schema.org/InStock",
  },
  author: {
    "@type": "Person",
    name: "Arthur Jean",
    url: "https://arthurjean.com",
    sameAs: ["https://github.com/ArthurDEV44"],
  },
  publisher: { "@type": "Organization", name: "StriveX", url: "https://strivex.fr" },
  codeRepository: "https://github.com/ArthurDEV44/rust-doctor",
  programmingLanguage: "Rust",
  license: "https://opensource.org/licenses/MIT",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html
      lang="en"
      className={`${geistSans.variable} ${geistMono.variable} h-full antialiased`}
      suppressHydrationWarning
    >
      <body className="min-h-full flex flex-col bg-background text-foreground font-sans overflow-x-hidden">
        <RootProvider
          theme={{
            attribute: "class",
            defaultTheme: "dark",
            enableSystem: true,
            disableTransitionOnChange: true,
          }}
        >
          <script
            type="application/ld+json"
            dangerouslySetInnerHTML={{
              __html: JSON.stringify(organizationJsonLd).replace(/</g, "\\u003c"),
            }}
          />
          <script
            type="application/ld+json"
            dangerouslySetInnerHTML={{
              __html: JSON.stringify(websiteJsonLd).replace(/</g, "\\u003c"),
            }}
          />
          <script
            type="application/ld+json"
            dangerouslySetInnerHTML={{
              __html: JSON.stringify(softwareJsonLd).replace(/</g, "\\u003c"),
            }}
          />
          {children}
        </RootProvider>
      </body>
    </html>
  );
}

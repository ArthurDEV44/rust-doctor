import type { ReactNode } from "react";
import { DocsLayout } from "fumadocs-ui/layouts/docs";
import { docsSource } from "@/lib/source";

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <DocsLayout
      tree={docsSource.pageTree}
      nav={{
        title: "rust-doctor",
        url: "/",
      }}
      links={[
        { text: "Blog", url: "/blog" },
        {
          text: "GitHub",
          url: "https://github.com/ArthurDEV44/rust-doctor",
          external: true,
        },
      ]}
    >
      {children}
    </DocsLayout>
  );
}

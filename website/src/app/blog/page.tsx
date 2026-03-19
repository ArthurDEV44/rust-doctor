import Link from "next/link";
import { blogPosts } from "@/lib/source";
import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Blog | rust-doctor",
  description:
    "Articles about Rust code quality, static analysis, and rust-doctor updates.",
};

export default function BlogIndex() {
  const posts = [...blogPosts].sort((a, b) => {
    const dateA = a.date ?? "";
    const dateB = b.date ?? "";
    return dateB.localeCompare(dateA);
  });

  return (
    <main className="container max-w-2xl mx-auto px-4 py-16">
      <h1 className="text-3xl font-bold mb-2">Blog</h1>
      <p className="text-muted-foreground mb-10">
        Articles about Rust code quality, static analysis, and rust-doctor.
      </p>

      <div className="space-y-8">
        {posts.map((post) => (
          <article key={post.info.path.replace(/\.mdx?$/, "")}>
            <Link href={`/blog/${post.info.path.replace(/\.mdx?$/, "")}`} className="group block">
              <h2 className="text-xl font-semibold group-hover:underline">
                {post.title}
              </h2>
              {post.description && (
                <p className="text-muted-foreground mt-1">
                  {post.description}
                </p>
              )}
              <div className="text-sm text-muted-foreground/70 mt-2 flex gap-3">
                {post.date && <time>{post.date}</time>}
                {post.author && <span>{post.author}</span>}
              </div>
            </Link>
          </article>
        ))}
      </div>
    </main>
  );
}

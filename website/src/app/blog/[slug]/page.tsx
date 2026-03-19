import { blogPosts } from "@/lib/source";
import { notFound } from "next/navigation";
import defaultMdxComponents from "fumadocs-ui/mdx";
import Link from "next/link";

interface Props {
  params: Promise<{ slug: string }>;
}

function getPost(slug: string) {
  return blogPosts.find((p) => p.info.path.replace(/\.mdx?$/, "") === slug);
}

export default async function BlogPost({ params }: Props) {
  const { slug } = await params;
  const post = getPost(slug);

  if (!post) notFound();

  const MDX = post.body;

  return (
    <main className="container max-w-2xl mx-auto px-4 py-16">
      <Link
        href="/blog"
        className="text-sm text-muted-foreground hover:text-foreground mb-6 inline-block"
      >
        &larr; Back to blog
      </Link>

      <article>
        <header className="mb-8">
          <h1 className="text-3xl font-bold">{post.title}</h1>
          {post.description && (
            <p className="text-muted-foreground mt-2 text-lg">
              {post.description}
            </p>
          )}
          <div className="text-sm text-muted-foreground/70 mt-3 flex gap-3">
            {post.date && <time>{post.date}</time>}
            {post.author && <span>{post.author}</span>}
          </div>
        </header>

        <div className="prose prose-neutral dark:prose-invert max-w-none">
          <MDX components={{ ...defaultMdxComponents }} />
        </div>
      </article>
    </main>
  );
}

export function generateStaticParams() {
  return blogPosts.map((post) => ({
    slug: post.info.path.replace(/\.mdx?$/, ""),
  }));
}

export async function generateMetadata({ params }: Props) {
  const { slug } = await params;
  const post = getPost(slug);

  if (!post) return {};

  return {
    title: `${post.title} | rust-doctor blog`,
    description: post.description,
  };
}

// @ts-nocheck
import { browser } from 'fumadocs-mdx/runtime/browser';
import type * as Config from '../source.config';

const create = browser<typeof Config, import("fumadocs-mdx/runtime/types").InternalTypeConfig & {
  DocData: {
  }
}>();
const browserCollections = {
  blog: create.doc("blog", {"hello-rust-doctor.mdx": () => import("../content/blog/hello-rust-doctor.mdx?collection=blog"), "what-rust-doctor-catches-that-clippy-misses.mdx": () => import("../content/blog/what-rust-doctor-catches-that-clippy-misses.mdx?collection=blog"), }),
  docs: create.doc("docs", {"ci-cd.mdx": () => import("../content/docs/ci-cd.mdx?collection=docs"), "cli-reference.mdx": () => import("../content/docs/cli-reference.mdx?collection=docs"), "configuration.mdx": () => import("../content/docs/configuration.mdx?collection=docs"), "index.mdx": () => import("../content/docs/index.mdx?collection=docs"), "installation.mdx": () => import("../content/docs/installation.mdx?collection=docs"), "mcp-server.mdx": () => import("../content/docs/mcp-server.mdx?collection=docs"), "rules/async.mdx": () => import("../content/docs/rules/async.mdx?collection=docs"), "rules/error-handling.mdx": () => import("../content/docs/rules/error-handling.mdx?collection=docs"), "rules/framework.mdx": () => import("../content/docs/rules/framework.mdx?collection=docs"), "rules/index.mdx": () => import("../content/docs/rules/index.mdx?collection=docs"), "rules/performance.mdx": () => import("../content/docs/rules/performance.mdx?collection=docs"), "rules/security.mdx": () => import("../content/docs/rules/security.mdx?collection=docs"), }),
};
export default browserCollections;
// @ts-nocheck
import * as __fd_glob_15 from "../content/docs/rules/security.mdx?collection=docs"
import * as __fd_glob_14 from "../content/docs/rules/performance.mdx?collection=docs"
import * as __fd_glob_13 from "../content/docs/rules/index.mdx?collection=docs"
import * as __fd_glob_12 from "../content/docs/rules/framework.mdx?collection=docs"
import * as __fd_glob_11 from "../content/docs/rules/error-handling.mdx?collection=docs"
import * as __fd_glob_10 from "../content/docs/rules/async.mdx?collection=docs"
import * as __fd_glob_9 from "../content/docs/mcp-server.mdx?collection=docs"
import * as __fd_glob_8 from "../content/docs/installation.mdx?collection=docs"
import * as __fd_glob_7 from "../content/docs/index.mdx?collection=docs"
import * as __fd_glob_6 from "../content/docs/configuration.mdx?collection=docs"
import * as __fd_glob_5 from "../content/docs/cli-reference.mdx?collection=docs"
import * as __fd_glob_4 from "../content/docs/ci-cd.mdx?collection=docs"
import { default as __fd_glob_3 } from "../content/docs/rules/meta.json?collection=docs"
import { default as __fd_glob_2 } from "../content/docs/meta.json?collection=docs"
import * as __fd_glob_1 from "../content/blog/what-rust-doctor-catches-that-clippy-misses.mdx?collection=blog"
import * as __fd_glob_0 from "../content/blog/hello-rust-doctor.mdx?collection=blog"
import { server } from 'fumadocs-mdx/runtime/server';
import type * as Config from '../source.config';

const create = server<typeof Config, import("fumadocs-mdx/runtime/types").InternalTypeConfig & {
  DocData: {
  }
}>({"doc":{"passthroughs":["extractedReferences"]}});

export const blog = await create.doc("blog", "content/blog", {"hello-rust-doctor.mdx": __fd_glob_0, "what-rust-doctor-catches-that-clippy-misses.mdx": __fd_glob_1, });

export const docs = await create.docs("docs", "content/docs", {"meta.json": __fd_glob_2, "rules/meta.json": __fd_glob_3, }, {"ci-cd.mdx": __fd_glob_4, "cli-reference.mdx": __fd_glob_5, "configuration.mdx": __fd_glob_6, "index.mdx": __fd_glob_7, "installation.mdx": __fd_glob_8, "mcp-server.mdx": __fd_glob_9, "rules/async.mdx": __fd_glob_10, "rules/error-handling.mdx": __fd_glob_11, "rules/framework.mdx": __fd_glob_12, "rules/index.mdx": __fd_glob_13, "rules/performance.mdx": __fd_glob_14, "rules/security.mdx": __fd_glob_15, });
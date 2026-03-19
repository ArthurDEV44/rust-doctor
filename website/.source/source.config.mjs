// source.config.ts
import {
  defineDocs,
  defineCollections,
  defineConfig,
  frontmatterSchema
} from "fumadocs-mdx/config";
import { z } from "zod";
var docs = defineDocs({
  dir: "content/docs"
});
var blog = defineCollections({
  type: "doc",
  dir: "./content/blog",
  schema: frontmatterSchema.extend({
    author: z.string().default("Arthur Jean"),
    date: z.string().date(),
    tags: z.array(z.string()).optional()
  })
});
var source_config_default = defineConfig({});
export {
  blog,
  source_config_default as default,
  docs
};

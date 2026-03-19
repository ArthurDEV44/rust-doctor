import {
  defineDocs,
  defineCollections,
  defineConfig,
  frontmatterSchema,
} from "fumadocs-mdx/config";
import { z } from "zod";

export const docs = defineDocs({
  dir: "content/docs",
});

export const blog = defineCollections({
  type: "doc",
  dir: "./content/blog",
  schema: frontmatterSchema.extend({
    author: z.string().default("Arthur Jean"),
    date: z.string().date(),
    tags: z.array(z.string()).optional(),
  }),
});

export default defineConfig({});

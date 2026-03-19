import type { MetadataRoute } from "next";

export default function sitemap(): MetadataRoute.Sitemap {
  return [
    {
      url: "https://rust-doctor.dev",
      lastModified: new Date("2026-03-19"),
      changeFrequency: "weekly",
      priority: 1,
    },
    {
      url: "https://rust-doctor.dev/docs",
      lastModified: new Date("2026-03-19"),
      changeFrequency: "weekly",
      priority: 0.9,
    },
    {
      url: "https://rust-doctor.dev/blog",
      lastModified: new Date("2026-03-19"),
      changeFrequency: "weekly",
      priority: 0.8,
    },
  ];
}

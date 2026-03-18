import type { MetadataRoute } from "next";

export default function sitemap(): MetadataRoute.Sitemap {
  return [
    {
      url: "https://rust-doctor.dev",
      lastModified: new Date("2026-03-18"),
      changeFrequency: "weekly",
      priority: 1,
    },
  ];
}

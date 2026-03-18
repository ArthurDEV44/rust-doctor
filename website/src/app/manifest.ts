import type { MetadataRoute } from "next";

export default function manifest(): MetadataRoute.Manifest {
  return {
    name: "rust-doctor",
    short_name: "rust-doctor",
    description:
      "Scan Rust projects for security, performance, correctness, architecture, and dependency issues.",
    start_url: "/",
    display: "standalone",
    background_color: "#0d0d0d",
    theme_color: "#0d0d0d",
    icons: [
      {
        src: "/images/rusty-happy.png",
        sizes: "512x512",
        type: "image/png",
      },
    ],
  };
}

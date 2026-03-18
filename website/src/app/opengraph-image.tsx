import { ImageResponse } from "next/og";

export const alt = "rust-doctor — Rust code health scanner";
export const size = {
  width: 1200,
  height: 630,
};
export const contentType = "image/png";

export default async function Image() {
  return new ImageResponse(
    (
      <div
        style={{
          background: "#0d0d0d",
          width: "100%",
          height: "100%",
          display: "flex",
          flexDirection: "column",
          justifyContent: "center",
          padding: "60px 80px",
          fontFamily: "system-ui, sans-serif",
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "16px",
            marginBottom: "24px",
          }}
        >
          <span style={{ fontSize: 48, color: "#f97316" }}>&#9764;</span>
          <span
            style={{
              fontSize: 56,
              fontWeight: 700,
              color: "#f5f5f5",
              letterSpacing: "-1px",
            }}
          >
            rust-doctor
          </span>
        </div>
        <p
          style={{
            fontSize: 32,
            color: "#a3a3a3",
            lineHeight: 1.4,
            margin: 0,
            maxWidth: "900px",
          }}
        >
          Scan Rust projects for security, performance, correctness, and
          architecture issues. Get a 0-100 health score.
        </p>
        <div
          style={{
            display: "flex",
            gap: "12px",
            marginTop: "40px",
          }}
        >
          {["700+ lints", "18 AST rules", "CVE detection", "MCP server"].map(
            (tag) => (
              <span
                key={tag}
                style={{
                  background: "#262626",
                  color: "#d4d4d4",
                  padding: "8px 16px",
                  borderRadius: "8px",
                  fontSize: 20,
                }}
              >
                {tag}
              </span>
            )
          )}
        </div>
        <p
          style={{
            position: "absolute",
            bottom: "40px",
            right: "80px",
            fontSize: 22,
            color: "#525252",
            margin: 0,
          }}
        >
          rust-doctor.dev
        </p>
      </div>
    ),
    { ...size }
  );
}

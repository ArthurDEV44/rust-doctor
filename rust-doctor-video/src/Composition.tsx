import {
  AbsoluteFill,
  Img,
  Sequence,
  interpolate,
  spring,
  staticFile,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";
import { loadFont } from "@remotion/google-fonts/JetBrainsMono";

const { fontFamily } = loadFont("normal", {
  weights: ["400", "700"],
  subsets: ["latin"],
});

// ── Data ────────────────────────────────────────────────────────────────

interface Diagnostic {
  severity: "error" | "warning";
  message: string;
  count: number;
}

interface Scenario {
  diagnostics: Diagnostic[];
  score: number;
  label: string;
  errors: number;
  warnings: number;
  files: number;
  duration: string;
  image: string;
  color: string;
  barColor: string;
}

const BAD_SCENARIO: Scenario = {
  diagnostics: [
    { severity: "error", message: "Hardcoded secret in connection string", count: 2 },
    { severity: "error", message: "panic!() in library crate, return Result", count: 3 },
    { severity: "error", message: "block_on() inside async context → deadlock", count: 1 },
    { severity: "warning", message: ".unwrap() in production code", count: 14 },
    { severity: "warning", message: ".clone() on large struct in hot loop", count: 7 },
    { severity: "warning", message: "blocking I/O in async fn, use tokio::fs", count: 4 },
  ],
  score: 38,
  label: "Critical",
  errors: 6,
  warnings: 33,
  files: 24,
  duration: "3.2s",
  image: "rusty-sad.png",
  color: "#f87171",
  barColor: "#f87171",
};

const GOOD_SCENARIO: Scenario = {
  diagnostics: [
    { severity: "warning", message: ".unwrap() in test helper, use expect()", count: 2 },
  ],
  score: 91,
  label: "Great",
  errors: 0,
  warnings: 2,
  files: 24,
  duration: "2.4s",
  image: "rusty-happy.png",
  color: "#4ade80",
  barColor: "#4ade80",
};

// ── Helpers ─────────────────────────────────────────────────────────────

const COMMAND = "npx -y rust-doctor@latest .";

function typewriter(text: string, frame: number, charsPerFrame: number): string {
  const chars = Math.floor(frame * charsPerFrame);
  return text.slice(0, Math.min(chars, text.length));
}

// ── Sub-components ──────────────────────────────────────────────────────

const Cursor: React.FC = () => {
  const frame = useCurrentFrame();
  const visible = Math.floor(frame / 8) % 2 === 0;
  return (
    <span
      style={{
        display: "inline-block",
        width: 10,
        height: 20,
        backgroundColor: visible ? "#a3a3a3" : "transparent",
        marginLeft: 2,
        verticalAlign: "middle",
      }}
    />
  );
};

const TerminalHeader: React.FC = () => (
  <div style={{ display: "flex", gap: 8, marginBottom: 24 }}>
    <div style={{ width: 12, height: 12, borderRadius: "50%", backgroundColor: "#ef4444" }} />
    <div style={{ width: 12, height: 12, borderRadius: "50%", backgroundColor: "#eab308" }} />
    <div style={{ width: 12, height: 12, borderRadius: "50%", backgroundColor: "#22c55e" }} />
  </div>
);

const ProgressBar: React.FC<{ score: number; color: string; animProgress: number }> = ({
  score,
  color,
  animProgress,
}) => {
  const filled = Math.round(score / 5);
  const visibleFilled = Math.round(filled * animProgress);
  return (
    <div style={{ display: "flex", gap: 2, marginTop: 6 }}>
      {Array.from({ length: 20 }).map((_, i) => (
        <div
          key={i}
          style={{
            height: 10,
            flex: 1,
            borderRadius: 2,
            backgroundColor: i < visibleFilled ? color : "#262626",
          }}
        />
      ))}
    </div>
  );
};

// ── Scene: Scan run ─────────────────────────────────────────────────────

const ScanScene: React.FC<{ scenario: Scenario; withTyping?: boolean }> = ({ scenario, withTyping = false }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  // Typing phase (only if withTyping)
  const TYPING_END = withTyping ? Math.ceil(COMMAND.length / 1.2 + 8) : 0;

  // Phase timings
  const HEADER_START = TYPING_END;
  const DIAGS_START = HEADER_START + 15;
  const DIAG_INTERVAL = 6;
  const DIAGS_END = DIAGS_START + scenario.diagnostics.length * DIAG_INTERVAL;
  const FACE_START = DIAGS_END + 8;
  const SCORE_START = FACE_START + 10;
  const BAR_START = SCORE_START + 8;
  const SUMMARY_START = BAR_START + 15;

  // Header
  const showHeader = frame >= HEADER_START;

  // Diagnostics
  const diagCount = frame >= DIAGS_START
    ? Math.min(
        Math.floor((frame - DIAGS_START) / DIAG_INTERVAL) + 1,
        scenario.diagnostics.length,
      )
    : 0;

  // Face (crab image)
  const showFace = frame >= FACE_START;
  const faceSpring = spring({ frame: frame - FACE_START, fps, config: { damping: 12 } });

  // Score counter
  const showScore = frame >= SCORE_START;
  const scoreProgress = showScore
    ? Math.min((frame - SCORE_START) / 15, 1)
    : 0;
  const displayScore = Math.round(scenario.score * scoreProgress);

  // Bar
  const showBar = frame >= BAR_START;
  const barProgress = showBar
    ? Math.min((frame - BAR_START) / 12, 1)
    : 0;

  // Summary
  const showSummary = frame >= SUMMARY_START;
  const summaryOpacity = showSummary
    ? interpolate(frame - SUMMARY_START, [0, 8], [0, 1], { extrapolateRight: "clamp" })
    : 0;

  return (
    <div style={{ fontFamily, fontSize: 18, color: "#d4d4d4", lineHeight: 1.7 }}>
      {/* Command line */}
      <p style={{ color: "#737373" }}>
        {"$ "}
        <span style={{ color: "#d4d4d4" }}>
          {withTyping ? typewriter(COMMAND, frame, 1.2) : COMMAND}
        </span>
        {withTyping && frame < TYPING_END && <Cursor />}
      </p>

      {/* Header */}
      {showHeader && (
        <div style={{ marginTop: 12 }}>
          <p style={{ color: "#737373" }}>
            <span style={{ color: "#fb923c" }}>&#9764;</span> rust-doctor v0.1.4
          </p>
          <p style={{ color: "#525252", fontSize: 14 }}>
            Scanning 24 files...
          </p>
        </div>
      )}

      {/* Diagnostics */}
      {diagCount > 0 && <div style={{ height: 12 }} />}
      {scenario.diagnostics.slice(0, diagCount).map((diag, i) => (
        <p key={i} style={{ color: "#a3a3a3", fontSize: 16 }}>
          <span style={{ color: "#525252" }}>{"> "}</span>
          <span style={{ color: diag.severity === "error" ? "#f87171" : "#facc15" }}>
            {diag.severity === "error" ? "✕" : "!"}
          </span>{" "}
          {diag.message}{" "}
          <span style={{ color: "#525252" }}>({diag.count})</span>
        </p>
      ))}

      {/* Face + Score area */}
      {showFace && (
        <div style={{ display: "flex", alignItems: "center", gap: 20, marginTop: 20 }}>
          <Img
            src={staticFile(scenario.image)}
            style={{
              width: 80,
              height: 80,
              objectFit: "contain",
              transform: `scale(${faceSpring})`,
            }}
          />
          <div>
            {showScore && (
              <p style={{ margin: 0, fontSize: 22 }}>
                <span style={{ color: scenario.color, fontWeight: 700, fontSize: 28 }}>
                  {displayScore}
                </span>
                <span style={{ color: "#737373" }}> / 100 </span>
                <span style={{ color: scenario.color, fontWeight: 700 }}>
                  {scoreProgress >= 1 ? scenario.label : ""}
                </span>
              </p>
            )}
            {showBar && (
              <ProgressBar
                score={scenario.score}
                color={scenario.barColor}
                animProgress={barProgress}
              />
            )}
          </div>
        </div>
      )}

      {/* Summary */}
      {showSummary && (
        <p style={{ marginTop: 16, color: "#737373", opacity: summaryOpacity, fontSize: 16 }}>
          {scenario.errors > 0 && (
            <>
              <span style={{ color: "#f87171" }}>{scenario.errors} errors</span>
              {", "}
            </>
          )}
          <span style={{ color: "#facc15" }}>{scenario.warnings} warnings</span>
          {" across "}
          {scenario.files} files in {scenario.duration}
        </p>
      )}
    </div>
  );
};

// ── Scene: Intro command ────────────────────────────────────────────────

const IntroScene: React.FC = () => {
  const frame = useCurrentFrame();

  const typedText = typewriter(COMMAND, frame, 1.2);
  const isTyping = typedText.length < COMMAND.length;

  return (
    <AbsoluteFill
      style={{
        justifyContent: "center",
        alignItems: "center",
        backgroundColor: "#0a0a0a",
      }}
    >
      <p
        style={{
          fontFamily,
          fontSize: 48,
          fontWeight: 700,
          margin: 0,
        }}
      >
        <span style={{ color: "#737373" }}>$ </span>
        <span style={{ color: "#e5e5e5" }}>{typedText}</span>
        {isTyping && <Cursor />}
      </p>
    </AbsoluteFill>
  );
};

// ── Scene: Fix text ─────────────────────────────────────────────────────

const FixScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const scaleSpring = spring({ frame, fps, config: { damping: 12, stiffness: 150 } });
  const opacity = interpolate(frame, [0, 8], [0, 1], { extrapolateRight: "clamp" });

  return (
    <AbsoluteFill
      style={{
        justifyContent: "center",
        alignItems: "center",
        backgroundColor: "#0a0a0a",
      }}
    >
      <div
        style={{
          fontFamily,
          fontSize: 42,
          color: "#e5e5e5",
          fontWeight: 700,
          opacity,
          transform: `scale(${scaleSpring})`,
          textAlign: "center",
        }}
      >
        Fix with coding agent
      </div>
    </AbsoluteFill>
  );
};

// ── Main composition ────────────────────────────────────────────────────

export const RustDoctorDemo: React.FC = () => {
  const { fps } = useVideoConfig();

  // Timeline (12s = 360 frames at 30fps)
  const INTRO_DURATION = 60;    // 2s — command splash
  const SCENE1_DURATION = 145;  // ~4.8s — bad scan
  const FIX_DURATION = 45;      // 1.5s — "Fix with coding agent"
  const SCENE2_DURATION = 110;  // ~3.7s — good scan

  const s1 = INTRO_DURATION;
  const s2 = s1 + SCENE1_DURATION;
  const s3 = s2 + FIX_DURATION;

  return (
    <AbsoluteFill style={{ backgroundColor: "#0a0a0a" }}>
      {/* Scene 0: Intro command splash */}
      <Sequence durationInFrames={INTRO_DURATION} premountFor={fps}>
        <IntroScene />
      </Sequence>

      {/* Scene 1: Bad scan */}
      <Sequence from={s1} durationInFrames={SCENE1_DURATION} premountFor={fps}>
        <AbsoluteFill style={{ padding: 60 }}>
          <TerminalHeader />
          <ScanScene scenario={BAD_SCENARIO} />
        </AbsoluteFill>
      </Sequence>

      {/* Scene 2: Fix text */}
      <Sequence from={s2} durationInFrames={FIX_DURATION} premountFor={fps}>
        <FixScene />
      </Sequence>

      {/* Scene 3: Good scan */}
      <Sequence from={s3} durationInFrames={SCENE2_DURATION} premountFor={fps}>
        <AbsoluteFill style={{ padding: 60 }}>
          <TerminalHeader />
          <ScanScene scenario={GOOD_SCENARIO} withTyping />
        </AbsoluteFill>
      </Sequence>
    </AbsoluteFill>
  );
};

import {
  AbsoluteFill,
  Sequence,
  interpolate,
  spring,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";
import { loadFont } from "@remotion/google-fonts/JetBrainsMono";

const { fontFamily } = loadFont("normal", {
  weights: ["400", "700"],
  subsets: ["latin"],
});

// ── Helpers ─────────────────────────────────────────────────────────────

const COMMAND = "npx rust-doctor@latest setup";

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

// ── Scene: Intro command ────────────────────────────────────────────────

const IntroScene: React.FC = () => {
  const frame = useCurrentFrame();
  const typedText = typewriter(COMMAND, frame, 1.2);
  const isTyping = typedText.length < COMMAND.length;

  return (
    <AbsoluteFill
      style={{ justifyContent: "center", alignItems: "center", backgroundColor: "#0a0a0a" }}
    >
      <p style={{ fontFamily, fontSize: 44, fontWeight: 700, margin: 0 }}>
        <span style={{ color: "#737373" }}>$ </span>
        <span style={{ color: "#e5e5e5" }}>{typedText}</span>
        {isTyping && <Cursor />}
      </p>
    </AbsoluteFill>
  );
};

// ── Scene: Wizard mode selection ────────────────────────────────────────

const WizardScene: React.FC = () => {
  const frame = useCurrentFrame();

  // Phase timings
  const BANNER_START = 0;
  const PROMPT_START = 18;
  const OPTION1_START = 30;
  const OPTION2_START = 40;
  const CURSOR_MOVE = 55; // cursor moves to CLI + Skills

  const bannerOpacity = interpolate(frame - BANNER_START, [0, 8], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const promptOpacity = interpolate(frame - PROMPT_START, [0, 8], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const opt1Opacity = interpolate(frame - OPTION1_START, [0, 6], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const opt2Opacity = interpolate(frame - OPTION2_START, [0, 6], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const cliSelected = frame >= CURSOR_MOVE;

  return (
    <div style={{ fontFamily, fontSize: 17, color: "#d4d4d4", lineHeight: 1.8 }}>
      {/* Banner */}
      <div style={{ opacity: bannerOpacity }}>
        <p style={{ fontWeight: 700, color: "#e5e5e5", fontSize: 20, margin: 0 }}>
          rust-doctor setup
        </p>
        <p style={{ color: "#525252", fontSize: 14, margin: 0, marginTop: 2 }}>
          Configure rust-doctor for your AI coding agent
        </p>
      </div>

      {/* Prompt */}
      <div style={{ marginTop: 28, opacity: promptOpacity }}>
        <p style={{ color: "#4ade80", margin: 0, fontSize: 16 }}>
          {"? "}
          <span style={{ color: "#e5e5e5", fontWeight: 700 }}>
            How should your agent access rust-doctor?
          </span>
        </p>
      </div>

      {/* Options */}
      <div style={{ marginTop: 12 }}>
        <p style={{ margin: 0, marginTop: 4, opacity: opt1Opacity }}>
          <span style={{ color: cliSelected ? "#4ade80" : "#525252", marginRight: 8 }}>
            {cliSelected ? "❯" : " "}
          </span>
          <span style={{ color: cliSelected ? "#e5e5e5" : "#737373" }}>
            CLI + Skills
          </span>
          <span style={{ color: "#525252" }}>
            {" — Installs a skill file that guides your agent "}
          </span>
          {cliSelected && (
            <span style={{ color: "#4ade80", fontSize: 13 }}>(recommended)</span>
          )}
        </p>

        <p style={{ margin: 0, marginTop: 4, opacity: opt2Opacity }}>
          <span style={{ color: !cliSelected ? "#4ade80" : "#525252", marginRight: 8 }}>
            {!cliSelected ? "❯" : " "}
          </span>
          <span style={{ color: !cliSelected ? "#e5e5e5" : "#737373" }}>
            MCP Server
          </span>
          <span style={{ color: "#525252" }}>
            {" — Agent calls rust-doctor tools via MCP protocol"}
          </span>
        </p>
      </div>
    </div>
  );
};

// ── Scene: Detection + Installation ─────────────────────────────────────

interface AgentLine {
  name: string;
  description: string;
}

const AGENTS: AgentLine[] = [
  { name: "Claude Code", description: "Anthropic's CLI for Claude" },
  { name: "Cursor", description: "AI-first code editor" },
];

const INSTALL_LINES = [
  { agent: "Claude Code", path: "~/.claude/skills/rust-doctor/SKILL.md" },
  { agent: "Cursor", path: "~/.cursor/skills/rust-doctor/SKILL.md" },
];

const InstallScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  // Phase timings
  const DETECT_LABEL = 0;
  const AGENT_INTERVAL = 12;
  const INSTALL_START = 50;
  const INSTALL_INTERVAL = 20;

  const detectOpacity = interpolate(frame - DETECT_LABEL, [0, 8], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  return (
    <div style={{ fontFamily, fontSize: 17, color: "#d4d4d4", lineHeight: 1.8 }}>
      {/* Detection header */}
      <p style={{ color: "#737373", margin: 0, opacity: detectOpacity }}>
        Detected{" "}
        <span style={{ color: "#e5e5e5", fontWeight: 700 }}>2</span>
        {" agent(s):"}
      </p>

      {/* Agent list */}
      <div style={{ marginTop: 12 }}>
        {AGENTS.map((agent, i) => {
          const agentStart = DETECT_LABEL + 12 + i * AGENT_INTERVAL;
          const agentOpacity = interpolate(frame - agentStart, [0, 6], [0, 1], {
            extrapolateLeft: "clamp",
            extrapolateRight: "clamp",
          });
          const agentScale = spring({
            frame: Math.max(0, frame - agentStart),
            fps,
            config: { damping: 15, stiffness: 200 },
          });

          return (
            <p key={i} style={{ margin: 0, marginTop: 2, opacity: agentOpacity }}>
              <span style={{ color: "#4ade80", transform: `scale(${agentScale})`, display: "inline-block" }}>
                {"✓ "}
              </span>
              <span style={{ color: "#e5e5e5", fontWeight: 700 }}>{agent.name}</span>
              <span style={{ color: "#525252" }}>{" — "}{agent.description}</span>
            </p>
          );
        })}
      </div>

      {/* Installation lines */}
      <div style={{ marginTop: 24 }}>
        {INSTALL_LINES.map((line, i) => {
          const lineStart = INSTALL_START + i * INSTALL_INTERVAL;
          const lineOpacity = interpolate(frame - lineStart, [0, 6], [0, 1], {
            extrapolateLeft: "clamp",
            extrapolateRight: "clamp",
          });

          // "done" appears after a brief delay
          const doneStart = lineStart + 10;
          const showDone = frame >= doneStart;

          return (
            <p key={i} style={{ margin: 0, marginTop: 4, opacity: lineOpacity, fontSize: 16 }}>
              <span style={{ color: "#737373" }}>{"  Installing skill for "}</span>
              <span style={{ color: "#67e8f9" }}>{line.agent}</span>
              <span style={{ color: "#737373" }}>{" ... "}</span>
              {showDone && (
                <span style={{ color: "#4ade80", fontWeight: 700 }}>done</span>
              )}
            </p>
          );
        })}
      </div>
    </div>
  );
};

// ── Scene: Recap ────────────────────────────────────────────────────────

const RecapScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const titleSpring = spring({ frame, fps, config: { damping: 14, stiffness: 180 } });
  const titleOpacity = interpolate(titleSpring, [0, 0.5, 1], [0, 0.8, 1]);

  const FILES_START = 18;
  const TRY_START = 55;

  return (
    <div style={{ fontFamily, fontSize: 17, color: "#d4d4d4", lineHeight: 1.8 }}>
      {/* Setup complete */}
      <p
        style={{
          color: "#4ade80",
          fontWeight: 700,
          fontSize: 22,
          margin: 0,
          opacity: titleOpacity,
          transform: `scale(${titleSpring})`,
          transformOrigin: "left center",
        }}
      >
        Setup complete!
      </p>

      {/* Installed files */}
      <div style={{ marginTop: 20 }}>
        <p style={{ color: "#737373", margin: 0, fontSize: 15 }}>Installed files:</p>
        {INSTALL_LINES.map((line, i) => {
          const fileStart = FILES_START + i * 10;
          const fileOpacity = interpolate(frame - fileStart, [0, 8], [0, 1], {
            extrapolateLeft: "clamp",
            extrapolateRight: "clamp",
          });

          return (
            <p key={i} style={{ margin: 0, marginTop: 3, opacity: fileOpacity, fontSize: 15 }}>
              <span style={{ color: "#4ade80" }}>{"  ✓ "}</span>
              <span style={{ color: "#737373" }}>{line.path}</span>
              <span style={{ color: "#525252" }}>{" (Skill file)"}</span>
            </p>
          );
        })}
      </div>

      {/* Try asking */}
      {frame >= TRY_START && (
        <div style={{ marginTop: 24 }}>
          <p style={{ color: "#737373", margin: 0, fontSize: 15 }}>
            Your agent can now use rust-doctor via CLI commands.
          </p>
          <p style={{ color: "#525252", margin: 0, marginTop: 2, fontSize: 15 }}>
            {'Try asking: '}
            <span style={{ color: "#737373", fontStyle: "italic" }}>
              {'"Run rust-doctor on this project"'}
            </span>
          </p>
        </div>
      )}
    </div>
  );
};

// ── Main composition ────────────────────────────────────────────────────

export const SetupDemo: React.FC = () => {
  const { fps } = useVideoConfig();

  // Timeline: 12s = 360 frames at 30fps
  const INTRO_DURATION = 55;     // ~1.8s — command typewriter
  const WIZARD_DURATION = 80;    // ~2.7s — mode selection
  const INSTALL_DURATION = 120;  // 4.0s — detection + installation
  const RECAP_DURATION = 105;    // 3.5s — setup complete recap

  const s1 = INTRO_DURATION;
  const s2 = s1 + WIZARD_DURATION;
  const s3 = s2 + INSTALL_DURATION;

  return (
    <AbsoluteFill style={{ backgroundColor: "#0a0a0a" }}>
      {/* Scene 0: Intro — type the command */}
      <Sequence durationInFrames={INTRO_DURATION} premountFor={fps}>
        <IntroScene />
      </Sequence>

      {/* Scene 1: Wizard — mode selection */}
      <Sequence from={s1} durationInFrames={WIZARD_DURATION} premountFor={fps}>
        <AbsoluteFill style={{ padding: 60 }}>
          <TerminalHeader />
          <WizardScene />
        </AbsoluteFill>
      </Sequence>

      {/* Scene 2: Detection + Installation */}
      <Sequence from={s2} durationInFrames={INSTALL_DURATION} premountFor={fps}>
        <AbsoluteFill style={{ padding: 60 }}>
          <TerminalHeader />
          <InstallScene />
        </AbsoluteFill>
      </Sequence>

      {/* Scene 3: Recap */}
      <Sequence from={s3} durationInFrames={RECAP_DURATION} premountFor={fps}>
        <AbsoluteFill style={{ padding: 60 }}>
          <TerminalHeader />
          <RecapScene />
        </AbsoluteFill>
      </Sequence>
    </AbsoluteFill>
  );
};

"use client";

import { useState, useEffect, useCallback, useRef } from "react";
import Image from "next/image";
import { RotateCcwIcon } from "lucide-react";

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
  face: string;
  image: string;
  color: string;
  barColor: string;
}

const SCENARIOS: Scenario[] = [
  {
    diagnostics: [
      { severity: "error", message: "Hardcoded secret detected in database connection string", count: 2 },
      { severity: "error", message: "panic!() called in library crate, return Result instead", count: 3 },
      { severity: "error", message: "block_on() inside async context causes deadlock", count: 1 },
      { severity: "warning", message: ".unwrap() in production code, use ? or expect()", count: 14 },
      { severity: "warning", message: ".clone() on large struct inside hot loop", count: 7 },
      { severity: "warning", message: 'String::from("literal") is slower than "literal".to_owned()', count: 5 },
      { severity: "warning", message: "blocking I/O in async function, use tokio::fs instead", count: 4 },
      { severity: "warning", message: ".collect() then .iter(), iterate directly instead", count: 3 },
    ],
    score: 38,
    label: "Critical",
    errors: 6,
    warnings: 33,
    files: 24,
    duration: "3.2s",
    face: "┌─────┐\n│ x x │\n│  ▽  │\n└─────┘",
    image: "/images/rusty-sad.webp",
    color: "text-red-600 dark:text-red-400",
    barColor: "bg-red-600 dark:bg-red-400",
  },
  {
    diagnostics: [
      { severity: "warning", message: ".unwrap() in production code, use ? or expect()", count: 8 },
      { severity: "warning", message: ".clone() on large struct, consider borrowing", count: 4 },
      { severity: "warning", message: "blocking I/O in async function, use tokio::fs instead", count: 2 },
      { severity: "warning", message: "enum variant size differs by 200+ bytes, consider Box", count: 1 },
      { severity: "warning", message: "unnecessary heap allocation, use stack reference", count: 3 },
    ],
    score: 62,
    label: "Needs work",
    errors: 0,
    warnings: 18,
    files: 12,
    duration: "1.8s",
    face: "┌─────┐\n│ • • │\n│  ─  │\n└─────┘",
    image: "/images/rusty-perplex.webp",
    color: "text-yellow-600 dark:text-yellow-400",
    barColor: "bg-yellow-600 dark:bg-yellow-400",
  },
  {
    diagnostics: [
      { severity: "warning", message: ".unwrap() in test helper, consider expect() with message", count: 2 },
      { severity: "warning", message: "String::from() on literal, prefer .to_owned()", count: 1 },
    ],
    score: 91,
    label: "Great",
    errors: 0,
    warnings: 3,
    files: 18,
    duration: "2.4s",
    face: "┌─────┐\n│ ◠ ◠ │\n│  ▽  │\n└─────┘",
    image: "/images/rusty-happy.webp",
    color: "text-green-600 dark:text-green-400",
    barColor: "bg-green-600 dark:bg-green-400",
  },
];

type Phase =
  | "typing"
  | "loading"
  | "diagnostics"
  | "face"
  | "score"
  | "bar"
  | "summary"
  | "done"
  | "pause";

const PHASE_INDEX: Record<Phase, number> = {
  typing: 0, loading: 1, diagnostics: 2, face: 3,
  score: 4, bar: 5, summary: 6, done: 7, pause: 8,
};

export function Terminal() {
  const [scenarioIndex, setScenarioIndex] = useState(0);
  const [phase, setPhase] = useState<Phase>("typing");
  const [typedChars, setTypedChars] = useState(0);
  const [visibleDiags, setVisibleDiags] = useState(0);
  const [animatedScore, setAnimatedScore] = useState(0);
  const pauseTimerRef = useRef<NodeJS.Timeout | null>(null);
  const scrollRef = useRef<HTMLDivElement | null>(null);

  const scenario = SCENARIOS[scenarioIndex];
  const command = "npx -y rust-doctor@latest .";

  const pi = PHASE_INDEX[phase];
  const showFace = pi >= 3;
  const showScore = pi >= 4;
  const showBar = pi >= 5;
  const showSummary = pi >= 7;

  const resetAnimation = useCallback(() => {
    setPhase("typing");
    setTypedChars(0);
    setVisibleDiags(0);
    setAnimatedScore(0);
  }, []);

  const nextScenario = useCallback(() => {
    const next = (scenarioIndex + 1) % SCENARIOS.length;
    setScenarioIndex(next);
    resetAnimation();
  }, [scenarioIndex, resetAnimation]);

  const restart = useCallback(() => {
    if (pauseTimerRef.current) clearTimeout(pauseTimerRef.current);
    setScenarioIndex(0);
    resetAnimation();
  }, [resetAnimation]);

  // Typing
  useEffect(() => {
    if (phase !== "typing") return;
    if (typedChars < command.length) {
      const t = setTimeout(() => setTypedChars((c) => c + 1), 25 + Math.random() * 35);
      return () => clearTimeout(t);
    }
    const t = setTimeout(() => setPhase("loading"), 300);
    return () => clearTimeout(t);
  }, [phase, typedChars, command.length]);

  // Loading
  useEffect(() => {
    if (phase !== "loading") return;
    const t = setTimeout(() => setPhase("diagnostics"), 600);
    return () => clearTimeout(t);
  }, [phase]);

  // Diagnostics
  useEffect(() => {
    if (phase !== "diagnostics") return;
    if (visibleDiags < scenario.diagnostics.length) {
      const t = setTimeout(() => setVisibleDiags((d) => d + 1), 150 + Math.random() * 100);
      return () => clearTimeout(t);
    }
    const t = setTimeout(() => setPhase("face"), 300);
    return () => clearTimeout(t);
  }, [phase, visibleDiags, scenario.diagnostics.length]);

  // Face
  useEffect(() => {
    if (phase !== "face") return;
    const t = setTimeout(() => {
      setAnimatedScore(0);
      setPhase("score");
    }, 400);
    return () => clearTimeout(t);
  }, [phase]);

  // Score (animated counter)
  useEffect(() => {
    if (phase !== "score") return;
    const target = scenario.score;
    let current = 0;
    const step = Math.max(1, Math.floor(target / 20));
    const interval = setInterval(() => {
      current += step;
      if (current >= target) {
        current = target;
        clearInterval(interval);
        setTimeout(() => setPhase("bar"), 200);
      }
      setAnimatedScore(current);
    }, 30);
    return () => clearInterval(interval);
  }, [phase, scenario.score]);

  // Bar
  useEffect(() => {
    if (phase !== "bar") return;
    const t = setTimeout(() => setPhase("done"), 400);
    return () => clearTimeout(t);
  }, [phase]);

  // Done → auto-advance to next scenario
  useEffect(() => {
    if (phase !== "done") return;
    const t = setTimeout(() => setPhase("pause"), 300);
    return () => clearTimeout(t);
  }, [phase]);

  // Pause between scenarios
  useEffect(() => {
    if (phase !== "pause") return;
    pauseTimerRef.current = setTimeout(nextScenario, 2000);
    return () => {
      if (pauseTimerRef.current) clearTimeout(pauseTimerRef.current);
    };
  }, [phase, nextScenario]);

  // Auto-scroll to bottom like a real terminal
  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [phase, visibleDiags, animatedScore]);

  const barFilled = Math.round(scenario.score / 5);

  return (
    <div className="w-full max-w-2xl font-mono text-[12px] sm:text-[14px] md:text-[15px] leading-relaxed">
      {/* Scenario indicator */}
      <div className="flex gap-2 mb-6">
        {SCENARIOS.map((s, i) => (
          <div
            key={i}
            className={`h-1 flex-1 rounded-full transition-all duration-500 ${
              i === scenarioIndex
                ? i === 0
                  ? "bg-red-600 dark:bg-red-400"
                  : i === 1
                  ? "bg-yellow-600 dark:bg-yellow-400"
                  : "bg-green-600 dark:bg-green-400"
                : i < scenarioIndex
                ? "bg-neutral-300 dark:bg-neutral-700"
                : "bg-neutral-200 dark:bg-neutral-800"
            }`}
          />
        ))}
      </div>

      {/* Terminal output — fixed height, auto-scroll to bottom */}
      <div ref={scrollRef} className="space-y-1 h-[360px] sm:h-[420px] overflow-y-auto scrollbar-hide">
        {/* Command */}
        <p className="text-muted-foreground">
          ${" "}
          {command.slice(0, typedChars)}
          {phase === "typing" && (
            <span className="inline-block w-2 h-4 bg-foreground/40 align-middle animate-pulse ml-0.5" />
          )}
        </p>

        {/* Header */}
        {phase !== "typing" && (
          <>
            <p className="text-muted-foreground mt-4">
              <span className="text-orange-600 dark:text-orange-400">&#9764;</span> rust-doctor
            </p>
            <p className="text-muted-foreground/70">
              Scan, score, and fix your Rust codebase.
            </p>
          </>
        )}

        {/* Diagnostics */}
        {visibleDiags > 0 && <div className="h-2" />}
        {scenario.diagnostics.slice(0, visibleDiags).map((diag, i) => (
          <p key={`${scenarioIndex}-${i}`} className="text-foreground/70 break-words">
            <span className="text-muted-foreground select-none">{"> "}</span>
            <span
              className={
                diag.severity === "error" ? "text-red-600 dark:text-red-400" : "text-yellow-600 dark:text-yellow-400"
              }
            >
              {diag.severity === "error" ? "x" : "!"}
            </span>{" "}
            <span className="hidden sm:inline">{diag.message}</span>
            <span className="sm:hidden">{diag.message.length > 50 ? diag.message.slice(0, 50) + "…" : diag.message}</span>
            {" "}
            <span className="text-muted-foreground">({diag.count})</span>
          </p>
        ))}

        {/* Doctor face */}
        {showFace && (
          <div className="mt-6 mb-2">
            <Image
              src={scenario.image}
              alt={`Ferris the crab - ${scenario.label}`}
              width={112}
              height={112}
              className="object-contain w-16 h-16 sm:w-28 sm:h-28"
            />
          </div>
        )}

        {/* Score */}
        {showScore && (
          <p className="mt-2">
            <span className={`${scenario.color} font-bold`}>
              {animatedScore}
            </span>
            <span className="text-muted-foreground"> / 100 </span>
            <span className={scenario.color}>{scenario.label}</span>
          </p>
        )}

        {/* Progress bar */}
        {showBar && (
          <div className="flex gap-[2px] mt-1">
            {Array.from({ length: 20 }).map((_, i) => (
              <div
                key={i}
                className={`h-2 flex-1 rounded-sm transition-all duration-300 ${
                  i < barFilled ? scenario.barColor : "bg-neutral-200 dark:bg-neutral-800"
                }`}
                style={{
                  transitionDelay: `${i * 25}ms`,
                }}
              />
            ))}
          </div>
        )}

        {/* Summary */}
        {showSummary && (
          <p className="mt-3 text-muted-foreground">
            {scenario.errors > 0 && (
              <>
                <span className="text-red-600 dark:text-red-400">{scenario.errors} errors</span>
                {", "}
              </>
            )}
            <span className="text-yellow-600 dark:text-yellow-400">
              {scenario.warnings} warnings
            </span>
            {" across "}
            {scenario.files} files in {scenario.duration}
          </p>
        )}

        {/* Auto-advance hint */}
        {phase === "pause" && (
          <p className="mt-4 text-muted-foreground/50 text-xs animate-pulse">
            Next scan...
          </p>
        )}
      </div>

      {/* Restart */}
      <div className="mt-4 flex justify-end">
        <button
          onClick={restart}
          className="flex items-center gap-1 text-xs text-muted-foreground/50 hover:text-muted-foreground transition-colors"
        >
          <RotateCcwIcon className="size-3" />
          Restart
        </button>
      </div>
    </div>
  );
}

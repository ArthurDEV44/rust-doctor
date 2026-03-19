const GRADES = [
  { range: "75–100", label: "Great", color: "text-green-500" },
  { range: "50–74", label: "Needs work", color: "text-yellow-500" },
  { range: "0–49", label: "Critical", color: "text-red-500" },
] as const;

export function ScoreSection() {
  return (
    <section className="max-w-3xl mx-auto px-4 sm:px-6 py-16 sm:py-24 border-t border-border/30">
      <p className="text-xs uppercase tracking-[0.08em] text-muted-foreground mb-3">
        Scoring
      </p>
      <h2 className="text-2xl sm:text-3xl font-semibold tracking-[-0.03em] text-foreground mb-4">
        One number. Zero ambiguity.
      </h2>
      <p className="text-sm text-muted-foreground leading-relaxed max-w-xl mb-10">
        Score = 100 &minus; unique error rules &times; 1.5 &minus; unique
        warning rules &times; 0.75. Counts unique rules violated, not total
        occurrences. Fix all instances of one issue — the entire penalty
        disappears.
      </p>

      <div className="grid grid-cols-3 gap-4">
        {GRADES.map((grade) => (
          <div
            key={grade.label}
            className="border border-border/50 rounded-md p-4 text-center"
          >
            <p className={`text-2xl sm:text-3xl font-bold ${grade.color}`}>
              {grade.range}
            </p>
            <p className="text-sm text-muted-foreground mt-1">{grade.label}</p>
          </div>
        ))}
      </div>
    </section>
  );
}

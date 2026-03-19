import { AST_RULES } from "@/lib/data";

export function RulesSection() {
  return (
    <section className="max-w-3xl mx-auto px-4 sm:px-6 py-16 sm:py-24 border-t border-border/30">
      <p className="text-xs uppercase tracking-[0.08em] text-muted-foreground mb-3">
        Rules
      </p>
      <h2 className="text-2xl sm:text-3xl font-semibold tracking-[-0.03em] text-foreground mb-10">
        19 custom AST rules.
      </h2>

      {/* Mobile: stacked rows */}
      <div className="space-y-px sm:hidden">
        {AST_RULES.map(([cat, rule, sev]) => (
          <div
            key={rule}
            className="flex items-center justify-between gap-3 py-3 border-b border-border/30"
          >
            <div className="min-w-0">
              <p className="text-xs text-muted-foreground/60">{cat}</p>
              <code className="text-sm text-foreground">{rule}</code>
            </div>
            <span
              className={`text-xs shrink-0 ${
                sev === "Error"
                  ? "text-red-500"
                  : sev === "Warning"
                    ? "text-yellow-500"
                    : "text-blue-500"
              }`}
            >
              {sev}
            </span>
          </div>
        ))}
      </div>

      {/* Desktop: table */}
      <div className="hidden sm:block">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border/50 text-xs uppercase tracking-[0.06em] text-muted-foreground/60">
              <th className="py-3 pr-4 text-left font-normal">Category</th>
              <th className="py-3 pr-4 text-left font-normal">Rule</th>
              <th className="py-3 text-left font-normal">Severity</th>
            </tr>
          </thead>
          <tbody>
            {AST_RULES.map(([cat, rule, sev]) => (
              <tr key={rule} className="border-b border-border/20">
                <td className="py-3 pr-4 text-muted-foreground">{cat}</td>
                <td className="py-3 pr-4">
                  <code className="text-foreground">{rule}</code>
                </td>
                <td className="py-3">
                  <span
                    className={
                      sev === "Error"
                        ? "text-red-500"
                        : sev === "Warning"
                          ? "text-yellow-500"
                          : "text-blue-500"
                    }
                  >
                    {sev}
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}

import { Badge } from "@/components/ui/badge";
import { Kbd } from "@/components/ui/kbd";
import { AST_RULES } from "@/lib/data";

export function RulesSection() {
  return (
    <section className="mb-12">
      <h2 className="text-lg sm:text-xl md:text-2xl font-semibold mb-4 font-sans text-foreground">
        18 custom AST rules
      </h2>

      {/* Mobile: card layout */}
      <div className="space-y-2 sm:hidden">
        {AST_RULES.map(([cat, rule, sev]) => (
          <div key={rule} className="flex items-center justify-between gap-2 border border-border rounded-lg p-3">
            <div className="min-w-0">
              <span className="text-muted-foreground text-xs">{cat}</span>
              <div className="truncate"><Kbd>{rule}</Kbd></div>
            </div>
            <Badge variant={sev === "Error" ? "error" : "warning"} size="sm" className="shrink-0">
              {sev}
            </Badge>
          </div>
        ))}
      </div>

      {/* Desktop: table layout */}
      <div className="hidden sm:block overflow-x-auto">
        <table className="w-full text-sm text-left">
          <thead>
            <tr className="border-b border-border text-muted-foreground">
              <th className="py-2 pr-4">Category</th>
              <th className="py-2 pr-4">Rule</th>
              <th className="py-2">Severity</th>
            </tr>
          </thead>
          <tbody className="text-muted-foreground">
            {AST_RULES.map(([cat, rule, sev]) => (
              <tr key={rule} className="border-b border-border">
                <td className="py-2 pr-4 text-muted-foreground">{cat}</td>
                <td className="py-2 pr-4">
                  <Kbd>{rule}</Kbd>
                </td>
                <td className="py-2">
                  <Badge variant={sev === "Error" ? "error" : "warning"} size="sm">
                    {sev}
                  </Badge>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}

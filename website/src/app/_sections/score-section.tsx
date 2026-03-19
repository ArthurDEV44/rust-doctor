import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";

export function ScoreSection() {
  return (
    <section className="mb-12">
      <h2 className="text-lg sm:text-xl md:text-2xl font-semibold mb-4 font-sans text-foreground">
        How is the health score calculated?
      </h2>
      <p className="text-muted-foreground mb-4">
        Score = 100 &minus; (unique error rules &times; 1.5) &minus; (unique
        warning rules &times; 0.75), clamped to 0&ndash;100. The score
        counts unique rules violated, not total occurrences. Fixing all
        instances of one issue removes the entire penalty.
      </p>
      <div className="grid grid-cols-3 gap-2 sm:gap-4 text-center text-xs sm:text-sm">
        <Card>
          <CardContent className="p-3">
            <Badge variant="success" size="lg">75&ndash;100</Badge>
            <div className="text-muted-foreground mt-1">Great</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-3">
            <Badge variant="warning" size="lg">50&ndash;74</Badge>
            <div className="text-muted-foreground mt-1">Needs work</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-3">
            <Badge variant="error" size="lg">0&ndash;49</Badge>
            <div className="text-muted-foreground mt-1">Critical</div>
          </CardContent>
        </Card>
      </div>
    </section>
  );
}

import {
  Accordion,
  AccordionItem,
  AccordionTrigger,
  AccordionPanel,
} from "@/components/ui/accordion";
import { FAQ_ITEMS } from "@/lib/data";

export function FaqSection() {
  return (
    <section className="max-w-3xl mx-auto px-4 sm:px-6 py-16 sm:py-24 border-t border-border/30">
      <p className="text-xs uppercase tracking-[0.08em] text-muted-foreground mb-3">
        FAQ
      </p>
      <h2 className="text-2xl sm:text-3xl font-semibold tracking-[-0.03em] text-foreground mb-10">
        Common questions.
      </h2>

      <Accordion>
        {FAQ_ITEMS.map((item) => (
          <AccordionItem key={item.question} value={item.question}>
            <AccordionTrigger>{item.question}</AccordionTrigger>
            <AccordionPanel>
              <p className="text-muted-foreground text-sm leading-relaxed">
                {item.answer}
              </p>
            </AccordionPanel>
          </AccordionItem>
        ))}
      </Accordion>
    </section>
  );
}

import {
  Accordion,
  AccordionItem,
  AccordionTrigger,
  AccordionPanel,
} from "@/components/ui/accordion";
import { FAQ_ITEMS } from "@/lib/data";

export function FaqSection() {
  return (
    <section className="mb-12">
      <h2 className="text-lg sm:text-xl md:text-2xl font-semibold mb-6 font-sans text-foreground">
        Frequently asked questions
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

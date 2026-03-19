import { Terminal } from "@/components/terminal";

export function HeroSection() {
  return (
    <div className="min-h-[100svh] bg-background flex justify-center items-start p-3 sm:p-4 md:p-8 pt-8 sm:pt-12 md:pt-20">
      <Terminal />
    </div>
  );
}

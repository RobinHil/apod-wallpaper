import { Download } from "./components/Download";
import { Faq } from "./components/Faq";
import { Features } from "./components/Features";
import { Footer } from "./components/Footer";
import { Hero } from "./components/Hero";
import { HowItWorks } from "./components/HowItWorks";
import { Modes } from "./components/Modes";
import { Nav } from "./components/Nav";
import { Starfield } from "./components/Starfield";
import { LocaleProvider } from "./i18n";
import type { Locale } from "./i18n/locale";
import { useApod } from "./useApod";

/**
 * The page.
 *
 * One fetch, shared: today's picture is asked for once and handed to the two
 * sections that show it, so a rate-limited demo key costs one refused request
 * and not three. It runs in the browser only, which is why the prerendered
 * HTML carries the painted sky and the real picture arrives on hydration.
 */
function Page() {
  const apod = useApod();

  return (
    <>
      <Starfield />
      <Nav />
      <main>
        <Hero apod={apod} />
        <Features />
        <HowItWorks />
        <Modes apod={apod} />
        <Download />
        <Faq />
      </main>
      <Footer />
    </>
  );
}

export function App({ locale }: { locale: Locale }) {
  return (
    <LocaleProvider locale={locale}>
      <Page />
    </LocaleProvider>
  );
}

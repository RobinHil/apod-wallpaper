import { useTranslation } from "../i18n";
import { LOCALES } from "../i18n/locale";

const LABELS = { en: "EN", fr: "FR" } as const;

/**
 * Two letters, two states, no dropdown: there are only two languages.
 *
 * They are links rather than buttons because each language is a page of its
 * own: the address bar, a bookmark and a shared link all name the language
 * they show, and a crawler follows them.
 */
export function LocaleSwitch() {
  const { locale, copy, hrefFor, choose } = useTranslation();

  return (
    <div
      className="inline-flex overflow-hidden rounded-[10px] border border-border bg-plate"
      role="group"
      aria-label={copy.nav.language}
    >
      {LOCALES.map((option) => (
        <a
          key={option}
          href={hrefFor(option)}
          lang={option}
          hrefLang={option}
          aria-current={option === locale ? "true" : undefined}
          onClick={() => choose(option)}
          className={`px-[10px] py-[8px] text-[12px] font-semibold transition ${
            option === locale
              ? "bg-accent-soft text-text"
              : "text-text-dim hover:text-text"
          }`}
        >
          {LABELS[option]}
        </a>
      ))}
    </div>
  );
}

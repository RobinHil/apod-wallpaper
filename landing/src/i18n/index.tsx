import { createContext, useContext, useEffect, useMemo, type ReactNode } from "react";
import { localePath } from "../paths";
import { en, type Copy } from "./en";
import { fr } from "./fr";
import { preferred, remember, remembered, type Locale } from "./locale";

export type { Locale } from "./locale";

const DICTIONARIES: Record<Locale, Copy> = { en, fr };

/** The copy for one language, which is what the prerender asks for too. */
export function copyFor(locale: Locale): Copy {
  return DICTIONARIES[locale];
}

interface Translation {
  locale: Locale;
  copy: Copy;
  /** Where the other language lives, as an href rather than a state change. */
  hrefFor: (locale: Locale) => string;
  /** Called when a visitor picks a language, before the browser navigates. */
  choose: (locale: Locale) => void;
}

const Context = createContext<Translation | null>(null);

/**
 * The language is decided by the URL, not by the page.
 *
 * Each language is prerendered at its own address, so what arrives is already
 * in the right language and there is nothing to swap at runtime. The one
 * thing left to do here is to send a visitor who landed on the wrong one to
 * the right one: a French browser opening the English root goes to `/fr/`,
 * once, and only while that visitor has never chosen a language by hand.
 */
export function LocaleProvider({
  locale,
  children,
}: {
  locale: Locale;
  children: ReactNode;
}) {
  useEffect(() => {
    if (remembered() !== null) return;

    const wanted = preferred();
    if (wanted === locale) return;

    // `replace` rather than `assign`: the page they did not want does not
    // belong in their history, where Back would land on it again.
    window.location.replace(localePath(wanted) + window.location.hash);
  }, [locale]);

  const value = useMemo<Translation>(
    () => ({
      locale,
      copy: DICTIONARIES[locale],
      hrefFor: localePath,
      choose: remember,
    }),
    [locale],
  );

  return <Context.Provider value={value}>{children}</Context.Provider>;
}

export function useTranslation(): Translation {
  const value = useContext(Context);
  if (!value) throw new Error("useTranslation used outside LocaleProvider");
  return value;
}

/** The copy alone, which is what most components need. */
export function useCopy(): Copy {
  return useTranslation().copy;
}

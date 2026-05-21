import i18n, { TFunction } from 'i18next';
import LanguageDetector from 'i18next-browser-languagedetector';
import { initReactI18next } from 'react-i18next';
import en from './en.json';

export abstract class ErrorI18n extends Error {
  abstract i18nKey(): TranslationKey | string;
}

export type TranslationKey = keyof typeof en;
export const defaultNS = 'translations';
export const resources = {
  en: {
    [defaultNS]: en
  }
};

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    // we init with resources
    resources,
    fallbackLng: 'en',
    debug: false,
    ns: [defaultNS],
    defaultNS: defaultNS,
    // by default ".". "if working with a flat JSON, it's recommended to set this to false"
    keySeparator: false,
    interpolation: {
      escapeValue: false
    }
  });

export default i18n;

export function fmtErrorI18n(t: TFunction, error: ErrorI18n): string {
  return t(error.i18nKey() as TranslationKey);
}

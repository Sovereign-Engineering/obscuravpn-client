import i18n from 'i18next';
import LanguageDetector from 'i18next-browser-languagedetector';
import { initReactI18next } from 'react-i18next';
import en from './en.json';

export const defaultNS = 'translations';
export const resources = {
  en: {
    translations: en
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

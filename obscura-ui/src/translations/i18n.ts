import i18n, { TFunction } from 'i18next';
import LanguageDetector from 'i18next-browser-languagedetector';
import { ReactNode } from 'react';
import { initReactI18next } from 'react-i18next';
import { CommandError } from '../bridge/commands';
import en from './en.json';

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

export function fmtVpnError(t: TFunction, errorCode: string): ReactNode {
  return t(`vpnError-${errorCode}` as TranslationKey);
}

// all errors over the bridge are CommandError's, see "ipcError-*" keys
export function fmtErrorI18n(t: TFunction, error: CommandError): ReactNode {
  return t(error.i18nKey() as TranslationKey);
}

// @ts-nocheck
import { TFunction } from 'i18next';
import { ReactNode } from 'react';
import { CommandError } from '../bridge/commands';

// no-check since errorCode is defined in Rust
// see "vpnError-*" keys
export function fmtVpnError(t: TFunction, errorCode: string): ReactNode {
  return t(`vpnError-${errorCode}`);
}

// all errors over the bridge are CommandError's, see "ipcError-*" keys
export function fmtErrorI18n(t: TFunction, error: CommandError): ReactNode {
  return t(error.i18nKey());
}

export function tUnsafe(t, key: string): ReactNode {
  return t(value);
}

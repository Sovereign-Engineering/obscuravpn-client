import { getCountryData, ICountryData, TContinentCode, TCountryCode } from "countries-list";
import { useEffect, useReducer } from 'react';
import { AccountId } from "./accountUtils";
import { AccountStatus } from './appContext';

export interface Exit {
    id: string,
    country_code: string, // lowercase TCountryCode
    city_code: string,
    city_name: string,
    provider_id: string,
    provider_url: string,
    provider_name: string,
    provider_homepage_url: string,
}

export function getContinent(countryData: ICountryData): TContinentCode {
  if (countryData.iso2 === 'MX') return 'SA';
  return countryData.continent;
}

export function getCountry(country_code: string): ICountryData {
  return getCountryData(country_code.toUpperCase() as TCountryCode);
}

export function getExitCountry(exit: Exit): ICountryData {
  if (exit.country_code.length !== 2) {
    console.warn(`Exit ${exit.id} (${exit.city_name}) does not have a country code of length 2 (got ${exit.country_code})`);
  }
  return getCountry(exit.country_code);
}

export interface AccountInfo {
    id: AccountId,
    active: boolean,
    top_up: TopUpInfo | null,
    subscription: SubscriptionInfo | null,
    apple_subscription: AppleSubscriptionInfo | null,
    auto_renews: number | null,
    current_expiry: number | null,
}

export interface TopUpInfo {
    credit_expires_at: number,
}

export function hasCredit(accountInfo: AccountInfo | undefined): boolean {
    const expires = accountInfo?.top_up?.credit_expires_at || 0;
    return new Date(expires * 1000).getTime() > new Date().getTime();
}

export interface SubscriptionInfo {
    status: SubscriptionStatus,
    current_period_start: number,
    current_period_end: number,
    cancel_at_period_end: boolean,
}

// returns if a subscription is active, regardless about renewal status
export function hasActiveSubscription(account: AccountInfo): boolean {
    if (account.subscription?.status === SubscriptionStatus.ACTIVE
        || account.subscription?.status === SubscriptionStatus.TRIALING) {
        return true;
    }
    if (account.apple_subscription?.status === AppleSubscriptionStatus.ACTIVE
        && account.apple_subscription.renewal_date > new Date().getTime()) {
      return true;
    }
    return false;
}

export function isRenewing(account: AccountInfo): boolean {
  return account.auto_renews !== null;
}

/// Returns the end of the current payment period.
///
/// Note that if the account has a renewing subscription it can stay active for longer.
export function paidUntil(account: AccountInfo): Date | null {
  const autoRenewDate = account.auto_renews || 0;
  const currentExpiry = account.current_expiry || 0;
  const maxExpiry = Math.max(autoRenewDate, currentExpiry);
  return maxExpiry > 0 ? new Date(maxExpiry * 1000) : null;
}

export function accountIsExpired(accountInfo: AccountInfo): boolean {
  if (accountInfo.auto_renews) return false;
  return (accountInfo.active && accountInfo.current_expiry) ?
    new Date(accountInfo.current_expiry * 1000).getTime() < new Date().getTime() :
    true;
}

// TimeRemaining is represented in parts of a whole
export interface TimeRemaining {
    days: number;
    hours: number;
    minutes: number;
}

/// Returns a human representation of the time left on an account.
///
/// Note that there is funny rounding on this number, it MUST NOT be used for computation.
export function accountTimeRemaining(account: AccountInfo): TimeRemaining {
  const expiry = paidUntil(account);
  const remainingMs = expiry !== null ? expiry.getTime() - Date.now() : 0;
  let remainingSeconds = Math.floor(remainingMs / 1000);

  const days = Math.floor(remainingMs / 1000 / 3600 / 24);
  remainingSeconds -= days * 86400;

  const hours = Math.floor(remainingSeconds / 3600);
  remainingSeconds -= hours * 3600;

  const minutes = Math.floor(remainingSeconds / 60);

  return { days, hours, minutes };
}

/// https://docs.stripe.com/api/subscriptions/object#subscription_object-status
export const enum SubscriptionStatus {
    ACTIVE = "active",
    CANCELED = "canceled",
    INCOMPLETE = "incomplete",
    INCOMPLETE_EXPIRED = "incomplete_expired",
    PAST_DUE = "past_due",
    PAUSED = "paused",
    TRIALING = "trialing",
    UNPAID = "unpaid",
}

// https://developer.apple.com/documentation/appstoreserverapi/status
export const enum AppleSubscriptionStatus {
    ACTIVE = 1,
    EXPIRED = 2,
    BILLING_RETRY = 3,
    GRACE_PERIOD = 4,
    REVOKED = 5,
}

export interface AppleSubscriptionInfo {
    status: AppleSubscriptionStatus,
    auto_renew_status: boolean,
    renewal_date: number,
}

export function hasAppleSubscription(accountInfo: AccountInfo | undefined): boolean {
    const status = accountInfo?.apple_subscription?.status;
    return status === AppleSubscriptionStatus.ACTIVE
      || status === AppleSubscriptionStatus.GRACE_PERIOD;
}

/**
 * Force the component to re-render when an account is expected to expire
 */
export function useReRenderWhenExpired(account: AccountStatus | null) {
  const [, forceUpdate] = useReducer(x => x + 1, 0);

  useEffect(() => {
    if (account !== null) {
      const expiryDate = paidUntil(account.account_info);
      if (expiryDate !== null && !accountIsExpired(account.account_info)) {
        const timeoutId = setTimeout(forceUpdate, expiryDate.getTime() - (new Date()).getTime());
        return () => clearTimeout(timeoutId);
      }
    }
  }, [account?.last_updated_sec]);
}

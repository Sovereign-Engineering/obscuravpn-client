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
}

export interface TopUpInfo {
    credit_expires_at: number,
}

export interface SubscriptionInfo {
    status: SubscriptionStatus,
    current_period_start: number,
    current_period_end: number,
    cancel_at_period_end: boolean,
}

export function getActiveSubscription(account: AccountInfo): SubscriptionInfo | undefined {
    if (account.subscription?.status === SubscriptionStatus.ACTIVE
        || account.subscription?.status === SubscriptionStatus.TRIALING) {
        return account.subscription;
    }
}

export function isRenewing(account: AccountInfo): boolean {
    return !!account.subscription
        && !account.subscription.cancel_at_period_end;
}

/// Returns the end of the current payment period.
///
/// Note that if the account has a renewing subscription it can stay active for longer.
export function paidUntil(account: AccountInfo): Date | undefined {
    let subscriptionExpires = account.subscription?.current_period_end;
    let topupExpires = account.top_up?.credit_expires_at;
    if (!subscriptionExpires)
        return topupExpires ? new Date(topupExpires*1000) : undefined;
    if (!topupExpires)
        return subscriptionExpires ? new Date(subscriptionExpires*1000) : undefined;
    return new Date(Math.max(subscriptionExpires, topupExpires)*1000);
}

/**
 * Checks if the active field is false and whether the topup/subscription timestamps have elapsed
 */
export function accountIsExpired(accountInfo: AccountInfo): boolean {
  const accountPaidUntil = paidUntil(accountInfo);
  return !accountInfo.active || accountPaidUntil === undefined || accountPaidUntil.getTime() < new Date().getTime();
}

/// Returns a human representation of the number of days left on an account.
///
/// Note that there is funny rounding on this number, it MUST NOT be used for computation.
///
/// TODO: Get a better representation, for example switching to hours and minutes as the expiry comes closer.
export function paidUntilDays(account: AccountInfo): number {
    let expiry = paidUntil(account);
    if (!expiry) {
        return 0;
    }
    let remainingMs = +expiry - Date.now();
    let remainingD = remainingMs / 1000 / 3600 / 24;
    return Math.floor(remainingD);
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

/**
 * Force the component to re-render when an account is expected to expire
 */
export function useReRenderWhenExpired(account: AccountStatus | null) {
  const [, forceUpdate] = useReducer(x => x + 1, 0);

  useEffect(() => {
    if (account !== null) {
      const expiryDate = paidUntil(account.account_info);
      if (expiryDate !== undefined && !accountIsExpired(account.account_info)) {
        const timeoutId = setTimeout(forceUpdate, expiryDate.getTime() - (new Date()).getTime());
        return () => clearTimeout(timeoutId);
      }
    }
  }, [account?.last_updated_sec]);
}

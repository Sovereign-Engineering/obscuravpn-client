import { getCountryData, ICountryData, TContinentCode, TCountryCode } from "countries-list";
import { useEffect } from 'react';
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

// seconds since unix epoch
export interface ApiTimestamp {
    readonly ApiTimestampTag: unique symbol;
}

export function apiTimestampToDate(timestamp: ApiTimestamp): Date {
    return new Date((timestamp as any as number) * 1000);
}

export interface AccountInfo {
    id: AccountId,
    active: boolean,
    top_up: TopUpInfo | null,
    apple_subscription: AppleSubscriptionInfo | null,
    google_subscription: GoogleSubscriptionInfo | null,
    stripe_subscription: StripeSubscriptionInfo | null,
    auto_renews: ApiTimestamp | null,
    current_expiry: ApiTimestamp | null,
    free_month_on_next_payment: boolean,
    free_trial: FreeTrialInfo | null,
    has_paid: boolean,
    nnn: NnnCaseInfo | null,
    primary_funding: FundingMethod | null,
}

export const enum FundingMethod {
    APPLE_SUBSCRIPTION = "apple_subscription",
    FREE_TRIAL = "free_trial",
    GOOGLE_SUBSCRIPTION = "google_subscription",
    STRIPE_SUBSCRIPTION = "stripe_subscription",
    TOP_UP = "top_up",
    // The native client sends this value for a funding method that it does not know.
    UNKNOWN = "unknown",
}

export interface FreeTrialInfo {
    active: boolean,
    expires_at: ApiTimestamp,
}

export type NnnShareId = string;

export interface NnnCaseInfo {
    applied_months: number | null,
    claimed_months: number,
    share_id: NnnShareId,
}

export interface ServiceStatusOutput {
    free_trial: FreeTrialStatus,
    nnn: NnnStatus,
}

export interface FreeTrialStatus {
    available: boolean,
}

export interface NnnStatus {
    claimed_months_target: number,
    claimed_months_total: number,
    submissions_open: boolean,
}

export interface TopUpInfo {
    active: boolean,
    credit_expires_at: ApiTimestamp,
}

export function hasCredit(accountInfo: AccountInfo | undefined): boolean {
    const creditExpiresAt = accountInfo?.top_up?.credit_expires_at;
    return creditExpiresAt !== undefined && apiTimestampToDate(creditExpiresAt).getTime() > new Date().getTime();
}

export interface StripeSubscriptionInfo {
    active: boolean,
    status: StripeSubscriptionStatus,
    current_period_start: ApiTimestamp,
    current_period_end: ApiTimestamp,
    cancel_at_period_end: boolean,
}

// returns if a subscription is active, regardless about renewal status
export function hasActiveSubscription(account: AccountInfo): boolean {
    return hasStripeSubscription(account) || hasAppleSubscription(account) || hasGoogleSubscription(account);
}

// TimeRemaining is represented in parts of a whole
export interface TimeRemaining {
    days: number;
    hours: number;
    minutes: number;
}

/// Returns a human representation of the time left until a date. A date in the past has no time left.
///
/// Note that there is funny rounding on this number, it MUST NOT be used for computation.
export function timeRemainingUntil(date: Date): TimeRemaining {
  const remainingMs = Math.max(date.getTime() - Date.now(), 0);
  let remainingSeconds = Math.floor(remainingMs / 1000);

  const days = Math.floor(remainingMs / 1000 / 3600 / 24);
  remainingSeconds -= days * 86400;

  const hours = Math.floor(remainingSeconds / 3600);
  remainingSeconds -= hours * 3600;

  const minutes = Math.floor(remainingSeconds / 60);

  return { days, hours, minutes };
}

/// https://docs.stripe.com/api/subscriptions/object#subscription_object-status
export const enum StripeSubscriptionStatus {
    ACTIVE = "active",
    CANCELED = "canceled",
    INCOMPLETE = "incomplete",
    INCOMPLETE_EXPIRED = "incomplete_expired",
    PAST_DUE = "past_due",
    PAUSED = "paused",
    TRIALING = "trialing",
    UNPAID = "unpaid",
}

export function hasStripeSubscription(accountInfo: AccountInfo | undefined): boolean {
  return accountInfo?.stripe_subscription?.active === true;
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
    active: boolean,
    status: AppleSubscriptionStatus,
    auto_renew_status: boolean,
    renewal_date: ApiTimestamp,
}

export function hasAppleSubscription(accountInfo: AccountInfo | undefined): boolean {
    return accountInfo?.apple_subscription?.active === true;
}

// https://developers.google.com/android-publisher/api-ref/rest/v3/purchases.subscriptionsv2#SubscriptionState
// https://www.revenuecat.com/blog/engineering/google-play-lifecycle/
export const enum GoogleSubscriptionStatus {
  UNSPECIFIED = "SUBSCRIPTION_STATE_UNSPECIFIED",
  PENDING = "SUBSCRIPTION_STATE_PENDING",
  ACTIVE = "SUBSCRIPTION_STATE_ACTIVE",
  PAUSED = "SUBSCRIPTION_STATE_PAUSED",
  IN_GRACE_PERIOD = "SUBSCRIPTION_STATE_IN_GRACE_PERIOD",
  ON_HOLD = "SUBSCRIPTION_STATE_ON_HOLD",
  CANCELED = "SUBSCRIPTION_STATE_CANCELED",
  EXPIRED = "SUBSCRIPTION_STATE_EXPIRED",
  PENDING_PURCHASE_CANCELED = "SUBSCRIPTION_STATE_PENDING_PURCHASE_CANCELED",
}

export interface GoogleSubscriptionInfo {
  status: GoogleSubscriptionStatus,
  active: boolean,
  auto_renew_status: boolean,
  expires_at: ApiTimestamp | null,
}

export function hasGoogleSubscription(accountInfo: AccountInfo | undefined): boolean {
  return accountInfo?.google_subscription?.active === true;
}

const MAX_TIMEOUT_MS = 2 ** 31 - 1;
// The API can report an active account after current_expiry
const EXPIRED_RECHECK_MS = 5 * 60 * 1000;

/**
 * Poll the account when an active account is expected to expire
 */
export function usePollAccountAtExpiry(account: AccountStatus | null, pollAccount: () => Promise<void>) {
  useEffect(() => {
    const currentExpiry = account?.account_info.current_expiry ?? null;
    if (account === null || currentExpiry === null || !account.account_info.active) return;

    const expiryMs = apiTimestampToDate(currentExpiry).getTime();
    let timeoutId: ReturnType<typeof setTimeout>;
    const schedule = () => {
      const untilExpiryMs = expiryMs - Date.now();
      const delayMs = untilExpiryMs > 0 ? Math.min(untilExpiryMs, MAX_TIMEOUT_MS) : EXPIRED_RECHECK_MS;
      timeoutId = setTimeout(() => {
        if (Date.now() >= expiryMs) void pollAccount();
        schedule();
      }, delayMs);
    };
    schedule();
    return () => clearTimeout(timeoutId);
  }, [account?.last_updated_sec]);
}

import { AccountInfo, apiTimestampToDate, AppleSubscriptionInfo, AppleSubscriptionStatus, FundingMethod, GoogleSubscriptionInfo, GoogleSubscriptionStatus, StripeSubscriptionInfo, StripeSubscriptionStatus } from './api';
import type { TranslationKey } from '../translations/i18n';

const MIN_EXTRA_CREDIT_MONTHS = 1;

export type SubscriptionType = 'apple' | 'google' | 'stripe';

export type AccountPaymentState =
  // The account never paid.
  | { kind: 'notStarted', freeMonthOnNextPayment: boolean }
  // The account paid before and has no funds now.
  | { kind: 'expired', failedSubscription: SubscriptionType | undefined }
  | { kind: 'freeTrial', endsAt: Date }
  | { kind: 'renewing', renewsAt: Date, extraCreditMonths: number | undefined }
  // The user cancelled the subscription. The account stays active until the period ends.
  | { kind: 'cancelled', expiresAt: Date }
  // A top-up funds the account. The subscription starts when the credit ends.
  | { kind: 'subscriptionAfterTopUp', topUpEndsAt: Date }
  | { kind: 'expires', expiresAt: Date }
  // The account is active and the API gave no renewal date and no expiry date.
  | { kind: 'active' };

type InactiveAccountState = Extract<AccountPaymentState, { kind: 'notStarted' | 'expired' }>;

type SubscriptionState =
  | 'cancelled'
  | 'expired'
  | 'paused'
  | 'payment_failed'
  | 'payment_pending'
  | 'renewing'
  | 'unknown';

// The API fields `active`, `auto_renews`, `current_expiry` and `primary_funding` select the state
export function getAccountPaymentState(account: AccountInfo): AccountPaymentState {
  if (!account.active) {
    return inactiveState(account);
  }
  const subscriptionType = subscriptionTypeOfFunding(account.primary_funding);
  if (subscriptionType !== undefined) {
    return subscriptionFundedState(account, subscriptionType);
  }
  switch (account.primary_funding) {
    case FundingMethod.FREE_TRIAL:
      if (account.current_expiry !== null) {
        return { kind: 'freeTrial', endsAt: apiTimestampToDate(account.current_expiry) };
      }
      return summaryState(account);
    case FundingMethod.TOP_UP:
      return topUpFundedState(account);
    default:
      // The API can add a funding method that this client does not know.
      return summaryState(account);
  }
}

function subscriptionTypeOfFunding(funding: FundingMethod | null): SubscriptionType | undefined {
  switch (funding) {
    case FundingMethod.APPLE_SUBSCRIPTION:
      return 'apple';
    case FundingMethod.GOOGLE_SUBSCRIPTION:
      return 'google';
    case FundingMethod.STRIPE_SUBSCRIPTION:
      return 'stripe';
    default:
      return undefined;
  }
}

export interface InactiveAccountMessage {
  title: TranslationKey,
  subtitle: TranslationKey | undefined,
}

// The message for an account that is not active. Use it in a view that does not show the account status card.
export function inactiveAccountMessage(account: AccountInfo): InactiveAccountMessage {
  const state = inactiveState(account);
  if (state.kind === 'notStarted') {
    return { title: 'account-NotStarted', subtitle: state.freeMonthOnNextPayment ? 'account-ReferralBonus' : undefined };
  }
  return {
    title: state.failedSubscription === undefined ? 'account-Expired' : 'account-ExpiredPaymentFailed',
    subtitle: 'continueUsingObscura',
  };
}

function inactiveState(account: AccountInfo): InactiveAccountState {
  if (!account.has_paid) {
    return { kind: 'notStarted', freeMonthOnNextPayment: account.free_month_on_next_payment };
  }
  // For an inactive account `primary_funding` is the funding with the latest expiry.
  // That can be a top-up that ended after a failed subscription, so examine all subscriptions in that case only.
  // An old subscription that never funded the account is not a failed payment.
  const primary = subscriptionTypeOfFunding(account.primary_funding);
  let candidates: SubscriptionType[] = [];
  if (primary !== undefined) {
    candidates = [primary];
  } else if (account.primary_funding === FundingMethod.TOP_UP) {
    candidates = ['apple', 'google', 'stripe'];
  }
  return {
    kind: 'expired',
    failedSubscription: candidates.find(type => subscriptionStateOf(account, type) === 'payment_failed'),
  };
}

function subscriptionFundedState(account: AccountInfo, subscriptionType: SubscriptionType): AccountPaymentState {
  const subscriptionState = subscriptionStateOf(account, subscriptionType);
  if (subscriptionState === 'renewing' && account.auto_renews !== null) {
    return {
      kind: 'renewing',
      renewsAt: apiTimestampToDate(account.auto_renews),
      extraCreditMonths: extraCreditMonths(account),
    };
  }
  if (subscriptionState === 'cancelled' && account.current_expiry !== null) {
    return { kind: 'cancelled', expiresAt: apiTimestampToDate(account.current_expiry) };
  }
  return summaryState(account);
}

function topUpFundedState(account: AccountInfo): AccountPaymentState {
  if (account.stripe_subscription?.active === true && account.auto_renews !== null) {
    const topUpEnd = account.top_up?.credit_expires_at ?? account.auto_renews;
    return { kind: 'subscriptionAfterTopUp', topUpEndsAt: apiTimestampToDate(topUpEnd) };
  }
  if (account.current_expiry !== null) {
    return { kind: 'expires', expiresAt: apiTimestampToDate(account.current_expiry) };
  }
  return summaryState(account);
}

// Use the summary fields only.
function summaryState(account: AccountInfo): AccountPaymentState {
  if (account.auto_renews !== null) {
    return { kind: 'renewing', renewsAt: apiTimestampToDate(account.auto_renews), extraCreditMonths: undefined };
  }
  if (account.current_expiry !== null) {
    return { kind: 'expires', expiresAt: apiTimestampToDate(account.current_expiry) };
  }
  console.warn('active account without a renewal or expiry date');
  return { kind: 'active' };
}

// Credit can remain after the renewal date of a subscription, for example after a top-up or free months.
function extraCreditMonths(account: AccountInfo): number | undefined {
  if (account.auto_renews === null || account.top_up === null) {
    return undefined;
  }
  const months = fullMonthsBetween(apiTimestampToDate(account.auto_renews), apiTimestampToDate(account.top_up.credit_expires_at));
  return months < MIN_EXTRA_CREDIT_MONTHS ? undefined : months;
}

function fullMonthsBetween(start: Date, end: Date): number {
  const calendarMonths = (end.getFullYear() - start.getFullYear()) * 12 + end.getMonth() - start.getMonth();
  const lastFullMonthEnd = new Date(start);
  lastFullMonthEnd.setMonth(start.getMonth() + calendarMonths);
  return lastFullMonthEnd > end ? calendarMonths - 1 : calendarMonths;
}

function subscriptionStateOf(account: AccountInfo, subscriptionType: SubscriptionType): SubscriptionState | undefined {
  switch (subscriptionType) {
    case 'stripe':
      return account.stripe_subscription ? stripeSubscriptionState(account.stripe_subscription) : undefined;
    case 'apple':
      return account.apple_subscription ? appleSubscriptionState(account.apple_subscription) : undefined;
    case 'google':
      return account.google_subscription ? googleSubscriptionState(account.google_subscription) : undefined;
  }
}

function stripeSubscriptionState(subscription: StripeSubscriptionInfo): SubscriptionState {
  switch (subscription.status) {
    case StripeSubscriptionStatus.ACTIVE:
    case StripeSubscriptionStatus.TRIALING:
      // A cancelled subscription keeps its status until the period ends.
      return subscription.cancel_at_period_end ? 'cancelled' : 'renewing';
    case StripeSubscriptionStatus.CANCELED:
      return 'cancelled';
    case StripeSubscriptionStatus.PAST_DUE:
    case StripeSubscriptionStatus.UNPAID:
      return 'payment_failed';
    case StripeSubscriptionStatus.PAUSED:
      return 'paused';
    case StripeSubscriptionStatus.INCOMPLETE:
    case StripeSubscriptionStatus.INCOMPLETE_EXPIRED:
      return 'expired';
    default:
      return 'unknown';
  }
}

function appleSubscriptionState(subscription: AppleSubscriptionInfo): SubscriptionState {
  switch (subscription.status) {
    case AppleSubscriptionStatus.ACTIVE:
      // A cancelled subscription keeps its status until the period ends.
      return subscription.auto_renew_status ? 'renewing' : 'cancelled';
    case AppleSubscriptionStatus.REVOKED:
      return 'cancelled';
    case AppleSubscriptionStatus.GRACE_PERIOD:
    case AppleSubscriptionStatus.BILLING_RETRY:
      return 'payment_failed';
    case AppleSubscriptionStatus.EXPIRED:
      return 'expired';
    default:
      return 'unknown';
  }
}

function googleSubscriptionState(subscription: GoogleSubscriptionInfo): SubscriptionState {
  switch (subscription.status) {
    case GoogleSubscriptionStatus.ACTIVE:
      return subscription.auto_renew_status ? 'renewing' : 'cancelled';
    case GoogleSubscriptionStatus.CANCELED:
    case GoogleSubscriptionStatus.PENDING_PURCHASE_CANCELED:
      return 'cancelled';
    case GoogleSubscriptionStatus.IN_GRACE_PERIOD:
    case GoogleSubscriptionStatus.ON_HOLD:
      return 'payment_failed';
    case GoogleSubscriptionStatus.PENDING:
      return 'payment_pending';
    case GoogleSubscriptionStatus.PAUSED:
      return 'paused';
    case GoogleSubscriptionStatus.EXPIRED:
      return 'expired';
    default:
      return 'unknown';
  }
}

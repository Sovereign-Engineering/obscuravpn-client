import { Anchor, Box, Button, Divider, Group, Loader, Stack, Text, TextInput, UnstyledButton } from '@mantine/core';
import { useContext, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import * as commands from '../bridge/commands';
import * as ObscuraAccount from '../common/accountUtils';
import { AccountInfo, AppleSubscriptionStatus, GoogleSubscriptionStatus, hasActiveSubscription, hasAppleSubscription, hasGoogleSubscription, StripeSubscriptionStatus } from '../common/api';
import { AppContext, SubscriptionProductModel } from '../common/appContext';
import { TranslationKey } from '../translations/i18n';
import { ButtonLink } from './ButtonLink';
import { ConfirmationDialog } from './ConfirmationDialog';
import { showErrorNotification } from '../common/utils';
import { IoMdPricetag } from 'react-icons/io';

interface PaymentManagementSheetProps {
  opened: boolean;
  onClose: () => void;
}

export function PaymentManagementSheet({ opened, onClose }: PaymentManagementSheetProps) {
  const { t } = useTranslation();
  const { appStatus, accountLoading, pollAccount, isProcessingPayment, osStatus, setPaymentProcessing } = useContext(AppContext);

  useEffect(() => {
    void pollAccount();
  }, []);

  // When the iOS offer code redemption sheet is closed,
  // there is a return value of success or failure;
  // `offerCodeRedemptionSuccess` is set to true in success.
  // For a brief period of time after a successful redemption,
  // the account info will show expired which will confuse new users.
  // When this field is set to true, we know to show the processing UI.
  useEffect(() => {
    if (osStatus.offerCodeRedemptionSuccess === true && !appStatus.account?.account_info.active) {
      setPaymentProcessing(true);
    }
  }, [osStatus.offerCodeRedemptionSuccess, appStatus.account?.account_info.active, setPaymentProcessing]);

  if (isProcessingPayment) {
    return <ProcessingPaymentSheet opened={true}/>;
  }

  const externalPaymentsAllowed = osStatus.storeKit?.externalPaymentsAllowed || osStatus.playBilling === false

  return (
    <ConfirmationDialog
      opened={opened}
      onClose={onClose}
      drawerSize='lg'
      title={t('accountManagement')}
      drawerCloseButton
    >
      <Stack h='100%' justify='space-between' gap='md'>
        {appStatus.account?.account_info ? (
          <>
            <AccountInfoOverview accountInfo={appStatus.account.account_info} />
            {!hasAppleSubscription(appStatus.account.account_info)
              && !hasGoogleSubscription(appStatus.account.account_info)
              && (externalPaymentsAllowed || appStatus.account.account_info.active)
              && <ButtonLink href={ObscuraAccount.payUrl(appStatus.accountId)}>{t(appStatus.account.account_info.active ? 'manageOnWeb' : 'payOnWeb')}</ButtonLink>}
          </>
        ) : (accountLoading ? (
          < Stack align='center' justify='center' h={200} >
            <Loader size='sm' />
            <Text c='dimmed'>{t('account-loading')}</Text>
          </Stack >
        ) :
          (
            <Stack align='center' justify='center' h={200}>
              <Text c='dimmed'>{t('account-InfoUnavailable')}</Text>
            </Stack>
          ))}
      </Stack >
    </ConfirmationDialog >
  );
}

function ProcessingPaymentSheet({ opened }: { opened: boolean }) {
  const { t } = useTranslation();

  return (
    <ConfirmationDialog
      opened={opened}
      onClose={() => { }}
      drawerSize='lg'
      title={t('processingPaymentTitle')}
      drawerCloseButton={false}
      closeOnClickOutside={false}
      closeOnEscape={false}
      withCloseButton={false}
    >
      <Stack align='center' justify='center' h={300} gap='md'>
        <Loader size='lg' />
        <Text ta="center">{t('processingPaymentTitleMessage')}</Text>
      </Stack>
    </ConfirmationDialog>
  );
}

interface AccountInfoOverviewProps {
  accountInfo: AccountInfo;
}

function AccountInfoOverview({ accountInfo }: AccountInfoOverviewProps) {
  const sections = useBuildSections(accountInfo);

  return (
    <Stack gap='md'>
      {sections.map((section, sectionIndex) => (
        <Box key={sectionIndex}>
          <Stack gap='xs'>
            {section}
          </Stack>
          {sectionIndex < sections.length - 1 && <Divider mt='md' />}
        </Box>
      ))}
    </Stack>
  );
}

function InfoRow({ title, importance, dataBolded, data, dataColor }: RowProps) {
  return (
    <Group justify='space-between' wrap='nowrap'>
      <Text
        size={importance === 'high' ? 'md' : 'sm'}
        fw={importance === 'high' ? 700 : importance === 'medium' ? 500 : 400}
        c={importance === 'high' ? undefined : 'dimmed'}
      >
        {title}
      </Text>
      <Text
        size={importance === 'high' ? 'md' : 'sm'}
        fw={dataBolded ? 700 : importance === 'medium' ? 500 : 400}
        c={dataColor || 'dimmed'}
        ta='right'
      >
        {data}
      </Text>
    </Group>
  );
}

type Importance = 'high' | 'medium' | 'low';

interface RowProps {
  title: string;
  importance: Importance;
  data?: React.ReactElement | string;
  dataBolded?: boolean;
  dataColor?: string;
}

type Section = React.ReactElement[];

interface AppleSubscriptionProductCardProps {
  product: SubscriptionProductModel;
  subscribed: boolean;
}

function AppleSubscriptionProductCard({ product, subscribed }: AppleSubscriptionProductCardProps) {
  const { t } = useTranslation();
  const { pollAccount, setPaymentProcessing } = useContext(AppContext);
  const { execute: storeKitAssociateAccount } = commands.useCommand({ command: commands.storeKitAssociateAccount, showNotification: true, rethrow: true });
  const [preparingToRedeem, setPreparingToRedeem] = useState(false);
  const [purchasing, setPurchasing] = useState(false);

  const handlePurchase = async () => {
    if (purchasing) return;
    setPurchasing(true);
    try {
      const purchaseSuccessful = await commands.storeKitPurchaseSubscription();
      if (purchaseSuccessful) {
        console.log('Purchase flow completed, show payment processing UI.');
        setPaymentProcessing(true);
        await pollAccount();
      } else {
        // user dismissed payment sheet
      }
    } catch (e) {
      showErrorNotification(t, e, 'purchaseFailed');
    } finally {
      setPurchasing(false);
    }
  }

  return (
    <Stack ta='center' p='0'>
      <Stack gap='0'>
        <Text fw={700}>
          {product.displayName}
        </Text>
        <Text c='dimmed'>
          {product.description}
        </Text>
        <Group justify='center' wrap='nowrap' gap='xs'>
          <Text c='dimmed' fw={700}>
            {product.subscriptionPeriodFormatted}:
          </Text>
          <Text fw={600}>
            {product.renewalPrice ?? product.displayPrice}
          </Text>
        </Group>
      </Stack>
      <Button component='a'
        loaderProps={{ type: 'dots' }}
        loading={purchasing}
        onClick={subscribed ? undefined : handlePurchase}
        href={subscribed ? ObscuraAccount.APP_MANAGE_SUBSCRIPTION : undefined}>
        {subscribed ? t('Manage Subscription') : t('Subscribe In-app')}</Button>
      {!subscribed && (
        <Stack gap='0'>
          <Group justify='center' gap='xs'>
            <Text c='dimmed'>
              {t('Have a promo code?')}
            </Text>
            <UnstyledButton disabled={preparingToRedeem} td='underline' c='blue' fw='normal' onClick={
              async () => {
                setPreparingToRedeem(true);
                try {
                  await storeKitAssociateAccount();
                  // if successfully associated account, show the redemption sheet
                  await commands.showOfferCodeRedemption();
                } finally {
                  setPreparingToRedeem(false);
                }
              }
            }>{t('Redeem Code')}</UnstyledButton>
          </Group>
          <Anchor size='xs' td='underline' c='blue' onClick={commands.storeKitRestorePurchases}>{t('Restore Purchases')}</Anchor>
        </Stack>
      )}
    </Stack>
  );
}
interface GoogleSubscriptionProductCardProps {
  subscribed: boolean;
}

function GoogleSubscriptionProductCard({ subscribed }: GoogleSubscriptionProductCardProps) {
  const { t } = useTranslation();
  const { pollAccount, setPaymentProcessing } = useContext(AppContext);
  const [promoCode, setPromoCode] = useState('');
  const [purchasing, setPurchasing] = useState(false);

  const handlePurchase = async () => {
    if (purchasing) return;
    setPurchasing(true);
    try {
      const promoCodeClean = promoCode.trim();
      const purchaseSuccessful = await commands.playPurchaseSubscription(promoCodeClean.length !== 0 ?
        promoCodeClean
        : null);
      if (purchaseSuccessful) {
        console.log('Purchase flow completed, show payment processing UI.');
        setPaymentProcessing(true);
        await pollAccount();
      } else {
        // user dismissed payment sheet
      }
    } catch (e) {
      showErrorNotification(t, e, 'purchaseFailed');
    } finally {
      setPurchasing(false);
    }
  }

  return (
    <Stack gap='xs' p='0' ta='center'>
      <Button
        component='a'
        href={subscribed ? "https://play.google.com/store/account/subscriptions?sku=vpn_subscription_v1&package=net.obscura.vpnclientapp" : undefined}
        loaderProps={{ type: 'dots' }}
        loading={purchasing}
        onClick={subscribed ? undefined : handlePurchase}
      >
        {subscribed ? t('Manage Subscription') : t('Subscribe In-app')}</Button>
      {!subscribed && <TextInput
        autoCapitalize='characters'
        disabled={purchasing}
        leftSection={<IoMdPricetag />}
        onChange={(e) => setPromoCode(e.currentTarget.value)}
        placeholder={t('Have a promo code?')}
        radius='md'
        value={promoCode}
        w={{ base: '100%', xs: 'auto' }}
      />}
    </Stack>
  );
}

function useBuildSections(accountInfo: AccountInfo): Section[] {
  const { t } = useTranslation();
  const { osStatus } = useContext(AppContext);
  const appleSubscriptionProduct = osStatus.storeKit?.subscriptionProduct;
  const sections: Section[] = [];

  const formattedId = ObscuraAccount.formatPartialAccountId(ObscuraAccount.accountIdToString(accountInfo.id));
  sections.push([<InfoRow title={t('Account ID')} importance='high' data={formattedId} />,],);
  sections.push([<InfoRow title={t('Status')} importance='high' data={accountInfo.active ? t('Active') : t('Inactive')} dataColor={accountInfo.active ? 'green' : 'red'} />]);

  // Top Up Section
  if (accountInfo.top_up) {
    const topUpDate = new Date(accountInfo.top_up.credit_expires_at * 1000);
    sections.push(
      [
        <InfoRow title={t('Top Up')} importance='high' />,
        <InfoRow title={t('Expiration Date')} importance='medium' data={topUpDate.toLocaleDateString()} />
      ]);
  }

  // Stripe Subscription Section
  const sub = accountInfo.stripe_subscription;
  if (sub && sub.status !== StripeSubscriptionStatus.CANCELED) {
    sections.push([
      <InfoRow title={t('subscribedOnWeb')} importance='high' />,
      <InfoRow title={t('Status')} importance='medium' data={t(`stripeStatus-${sub.status}`)} dataColor={getStripeStatusColor(sub.status)} />,
      <InfoRow title={t('Source')} importance='medium' data={new URL(ObscuraAccount.OBSCURA_WEBPAGE).hostname} />,
      <InfoRow title={t('Period Start')} importance='medium' data={new Date(sub.current_period_start * 1000).toLocaleDateString()} />,
      <InfoRow title={t('Period End')} importance='medium' data={new Date(sub.current_period_end * 1000).toLocaleDateString()} />,
      <InfoRow title={t('cancelAtEnd')} importance='medium' data={sub.cancel_at_period_end ? t('Yes') : t('No')} />,
    ]);
  }

  if (accountInfo.apple_subscription) {
    const appleSub = accountInfo.apple_subscription;
    const section = [];
    if (appleSubscriptionProduct) {
      section.push(<AppleSubscriptionProductCard product={appleSubscriptionProduct} subscribed={hasAppleSubscription(accountInfo)} />);
    }
    section.push(
      <InfoRow title={t('Status')} importance='medium' data={t(appleStatusToTranslationKey(appleSub.status))} dataColor={getAppleSubscriptionStatusColor(appleSub.status)} />,
      <InfoRow title={t('Source')} importance='medium' data={t('App Store')} />,
      <InfoRow title={t('Auto-Renewal')} importance='medium' data={appleSub.auto_renew_status ? t('Enabled') : t('Disabled')} />,
    )
    if (appleSub.auto_renew_status) {
      section.push(<InfoRow title={t('Renewal Date')} importance='medium' data={new Date(appleSub.renewal_date * 1000).toLocaleDateString()} />);
    }
    sections.push(section);
  } else if (appleSubscriptionProduct && !hasActiveSubscription(accountInfo)) {
    sections.push([
      <AppleSubscriptionProductCard product={appleSubscriptionProduct} subscribed={false} />,
    ]);
  }

  if (accountInfo.google_subscription) {
    const googleSub = accountInfo.google_subscription;
    const section = [];
    if (osStatus.playBilling) {
      section.push(<GoogleSubscriptionProductCard subscribed={hasGoogleSubscription(accountInfo)} />);
    }
    section.push(
      <InfoRow title={t('Status')} importance='medium' data={t(googleStatusToTranslationKey(googleSub.status))} dataColor={getGoogleSubscriptionStatusColor(googleSub.status)} />,
      <InfoRow title={t('Source')} importance='medium' data={t('Play Store')} />,
      <InfoRow title={t('Auto-Renewal')} importance='medium' data={googleSub.auto_renew_status ? t('Enabled') : t('Disabled')} />,
    )
    if (googleSub.auto_renew_status && googleSub.expires_at !== null) {
      section.push(<InfoRow title={t('Renewal Date')} importance='medium' data={new Date(googleSub.expires_at * 1000).toLocaleDateString()} />);
    }
    sections.push(section);
  } else if (osStatus.playBilling && !hasActiveSubscription(accountInfo)) {
    sections.push([
      <GoogleSubscriptionProductCard subscribed={false} />,
    ]);
  }

  return sections;
}

function getStripeStatusColor(status: StripeSubscriptionStatus): string {
  switch (status) {
    case StripeSubscriptionStatus.ACTIVE:
    case StripeSubscriptionStatus.TRIALING:
      return 'green';
    case StripeSubscriptionStatus.PAST_DUE:
    case StripeSubscriptionStatus.INCOMPLETE:
    case StripeSubscriptionStatus.PAUSED:
      return 'yellow';
    case StripeSubscriptionStatus.CANCELED:
    case StripeSubscriptionStatus.UNPAID:
    case StripeSubscriptionStatus.INCOMPLETE_EXPIRED:
      return 'red';
    default:
      return 'gray';
  }
}

function getAppleSubscriptionStatusColor(status: AppleSubscriptionStatus): string {
  switch (status) {
    case AppleSubscriptionStatus.ACTIVE:
      return 'green';
    case AppleSubscriptionStatus.GRACE_PERIOD:
      return 'orange';
    case AppleSubscriptionStatus.BILLING_RETRY:
    case AppleSubscriptionStatus.EXPIRED:
    case AppleSubscriptionStatus.REVOKED:
      return 'red';
    default:
      return 'gray';
  }
}

function getGoogleSubscriptionStatusColor(status: GoogleSubscriptionStatus): string {
  switch (status) {
    case GoogleSubscriptionStatus.ACTIVE:
    case GoogleSubscriptionStatus.CANCELED:
      return 'green';
    case GoogleSubscriptionStatus.IN_GRACE_PERIOD:
      return 'orange';
    case GoogleSubscriptionStatus.EXPIRED:
    case GoogleSubscriptionStatus.ON_HOLD:
    case GoogleSubscriptionStatus.PAUSED:
      return 'red';
    default:
      return 'gray';
  }
}

function appleStatusToTranslationKey(status: AppleSubscriptionStatus): TranslationKey {
  switch (status) {
    case AppleSubscriptionStatus.ACTIVE:
      return 'appleStatus-active' as TranslationKey;
    case AppleSubscriptionStatus.EXPIRED:
      return 'appleStatus-expired' as TranslationKey;
    case AppleSubscriptionStatus.BILLING_RETRY:
      return 'appleStatus-billingRetry' as TranslationKey;
    case AppleSubscriptionStatus.GRACE_PERIOD:
      return 'appleStatus-gracePeriod' as TranslationKey;
    case AppleSubscriptionStatus.REVOKED:
      return 'appleStatus-revoked' as TranslationKey;
    default:
      return 'appleStatus-unknown' as TranslationKey;
  }
}

function googleStatusToTranslationKey(status: GoogleSubscriptionStatus): TranslationKey {
  switch (status) {
    case GoogleSubscriptionStatus.ACTIVE:
      return 'googleStatus-active' as TranslationKey;
    case GoogleSubscriptionStatus.CANCELED:
      return 'googleStatus-canceled' as TranslationKey;
    case GoogleSubscriptionStatus.EXPIRED:
      return 'googleStatus-expired' as TranslationKey;
    case GoogleSubscriptionStatus.IN_GRACE_PERIOD:
      return 'googleStatus-inGracePeriod' as TranslationKey;
    case GoogleSubscriptionStatus.ON_HOLD:
      return 'googleStatus-onHold' as TranslationKey;
    case GoogleSubscriptionStatus.PAUSED:
      return 'googleStatus-paused' as TranslationKey;
    default:
      return 'googleStatus-unknown' as TranslationKey;
  }
}

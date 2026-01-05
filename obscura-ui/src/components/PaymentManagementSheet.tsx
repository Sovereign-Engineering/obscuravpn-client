import { Anchor, Box, Button, Divider, Group, Loader, Stack, Text, UnstyledButton } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import { useContext, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import * as commands from '../bridge/commands';
import * as ObscuraAccount from '../common/accountUtils';
import { AccountInfo, activeAppleSubscription, AppleSubscriptionStatus, hasAppleSubscription, SubscriptionStatus } from '../common/api';
import { AppContext, SubscriptionProductModel } from '../common/appContext';
import { TranslationKey } from '../translations/i18n';
import { ButtonLink } from './ButtonLink';
import { ConfirmationDialog } from './ConfirmationDialog';

interface PaymentManagementSheetProps {
  opened: boolean;
  onClose: () => void;
}

export function PaymentManagementSheet({ opened, onClose }: PaymentManagementSheetProps) {
  const { t } = useTranslation();
  const { appStatus, accountLoading, pollAccount, isProcessingPayment } = useContext(AppContext);

  useEffect(() => {
    void pollAccount();
  }, []);

  if (isProcessingPayment) {
    return <ProcessingPaymentSheet opened={true} />;
  }

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
            {!activeAppleSubscription(appStatus.account.account_info) &&
              <ButtonLink href={ObscuraAccount.payUrl(appStatus.accountId)}>{t(appStatus.account.account_info.active ? 'manageOnWeb' : 'payOnWeb')}</ButtonLink>}
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
  const sections = buildSections(accountInfo);

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

interface SubscriptionProductCardProps {
  product: SubscriptionProductModel;
  subscribed: boolean;
}

function AppleSubscriptionProductCard({ product, subscribed }: SubscriptionProductCardProps) {
  const { t } = useTranslation();
  const { pollAccount, setPaymentProcessing } = useContext(AppContext);
  const commandHandler = commands.useHandleCommand(t);
  const redeemCodeLinkRef = useRef<HTMLAnchorElement>(null);
  const [preparingToRedeem, setPreparingToRedeem] = useState(false);

  const handlePurchase = async () => {
    try {
      const purchaseSuccessful = await commands.storeKitPurchaseSubscription();
      if (purchaseSuccessful) {
        console.log('Purchase flow completed, show payment processing UI.');
        setPaymentProcessing(true);
        await pollAccount();
      }
      // else: user dismissed payment sheet
    } catch {
      notifications.show({
        color: 'red',
        title: t('purchaseFailed'),
        message: t('purchaseFailedMessage')
      });
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
                  await commandHandler(commands.storeKitAssociateAccount);
                  // if successfully associated account, open the URL
                  redeemCodeLinkRef.current?.click();
                } finally {
                  setPreparingToRedeem(false);
                }
              }
            }>{t('Redeem Code')}</UnstyledButton>
            <Anchor ref={redeemCodeLinkRef} href={ObscuraAccount.APP_REDEEM_OFFER_CODE} style={{ display: 'none' }} />
          </Group>
          <Anchor size='xs' td='underline' c='blue' onClick={commands.storeKitRestorePurchases}>{t('Restore Purchases')}</Anchor>
        </Stack>
      )}
    </Stack>
  );
}

function buildSections(accountInfo: AccountInfo): Section[] {
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
  const sub = accountInfo.subscription;
  if (sub && sub.status !== SubscriptionStatus.CANCELED) {
    sections.push([
      <InfoRow title={t('subscribedOnWeb')} importance='high' />,
      <InfoRow title={t('Status')} importance='medium' data={t(`stripeStatus-${sub.status}`)} dataColor={getStripeStatusColor(sub.status)} />,
      <InfoRow title={t('Source')} importance='medium' data='obscura.net' />,
      <InfoRow title={t('Period Start')} importance='medium' data={new Date(sub.current_period_start * 1000).toLocaleDateString()} />,
      <InfoRow title={t('Period End')} importance='medium' data={new Date(sub.current_period_end * 1000).toLocaleDateString()} />,
      <InfoRow title={t('cancelAtEnd')} importance='medium' data={sub.cancel_at_period_end ? t('Yes') : t('No')} />,
    ]);
  }

  // Apple Subscription Section
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
  } else if (appleSubscriptionProduct && !accountInfo.active) {
    // Show subscription product if account is inactive
    sections.push([
      <AppleSubscriptionProductCard product={appleSubscriptionProduct} subscribed={false} />,
    ]);
  }

  return sections;
}

function getStripeStatusColor(status: SubscriptionStatus): string {
  switch (status) {
    case SubscriptionStatus.ACTIVE:
    case SubscriptionStatus.TRIALING:
      return 'green';
    case SubscriptionStatus.PAST_DUE:
    case SubscriptionStatus.INCOMPLETE:
    case SubscriptionStatus.PAUSED:
      return 'yellow';
    case SubscriptionStatus.CANCELED:
    case SubscriptionStatus.UNPAID:
    case SubscriptionStatus.INCOMPLETE_EXPIRED:
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

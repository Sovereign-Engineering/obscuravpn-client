import { Anchor, Box, Button, Center, Code, Group, Loader, Paper, Stack, Text, ThemeIcon, useMantineTheme } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import React, { useContext, useEffect } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { BsQuestionSquareFill } from 'react-icons/bs';
import { FaRotateRight } from 'react-icons/fa6';
import { IoLogOutOutline, IoNuclear } from 'react-icons/io5';
import { MdOutlineWifiOff } from 'react-icons/md';
import * as commands from '../bridge/commands';
import { IS_HANDHELD_DEVICE } from '../bridge/SystemProvider';
import * as ObscuraAccount from '../common/accountUtils';
import { AccountInfo, accountIsExpired, accountTimeRemaining, hasActiveSubscription, hasAppleSubscription, hasCredit, isRenewing, paidUntil, useReRenderWhenExpired } from '../common/api';
import { AppContext, NEVPNStatus } from '../common/appContext';
import commonClasses from '../common/common.module.css';
import { normalizeError } from '../common/utils';
import { AccountNumberSection } from '../components/AccountNumberSection';
import { ButtonLink } from '../components/ButtonLink';
import { ConfirmationDialog } from '../components/ConfirmationDialog';
import { PaymentManagementSheet } from '../components/PaymentManagementSheet';
import AccountExpiredBadge from '../res/account-expired.svg?react';
import PaidUpExpiringSoonBadge from '../res/paid-up-expiring-soon.svg?react';
import PaidUpExpiringVerySoonBadge from '../res/paid-up-expiring-very-soon.svg?react';
import PaidUpSubscriptionActive from '../res/paid-up-subscription-active.svg?react';
import PaidUpBadge from '../res/paid-up.svg?react';
import SubscriptionActiveBadge from '../res/subscription-active.svg?react';
import SubscriptionPausedBadge from '../res/subscription-paused.svg?react';
import { fmtErrorI18n } from '../translations/i18n';
import classes from './AccountView.module.css';

export default function Account() {
  const { t } = useTranslation();
  const { appStatus, pollAccount, osStatus } = useContext(AppContext);
  const [confirmDeleteAccount, { open: openDeleteAccount, close: closeDeleteAccount }] = useDisclosure(false);
  const [confirmLogOut, { open: openLogOutConfirm, close: closeLogOutConfirm }] = useDisclosure(false);
  const osVpnConnected = osStatus.osVpnStatus === NEVPNStatus.Connected;

  useEffect(() => {
    // Ensure account info is up-to-date when the user is viewing the account page.
    void pollAccount();
  }, []);

  const logOut = async () => {
    try {
      await commands.disconnect();
      await commands.logout();
    } catch (e) {
      const error = normalizeError(e);
      notifications.show({ title: t('logOutFailed'), message: <Text>{t('pleaseReportError')}<br /><Code>{error.message}</Code></Text> });
    } finally {
      closeLogOutConfirm();
    }
  }

  const deleteAccount = async () => {
    try {
      await commands.deleteAccount();
      await logOut();
    } catch (e) {
      const error = normalizeError(e);
      notifications.show({ title: t('deleteAccountFailed'), message: <Text>{t('pleaseReportError')}<br /><Code>{error.message}</Code></Text>, color: 'red' });
    }
  }

  // vpnStatus is used because accountInfo will be null if pollAccount fails
  const accountId = appStatus.accountId;
  const accountInfo = appStatus.account?.account_info;
  return <>
    <ConfirmationDialog opened={confirmDeleteAccount} onClose={closeDeleteAccount} drawerSize='md'>
      <Stack h='100%' justify='space-between'>
        <Stack p={IS_HANDHELD_DEVICE ? 'xl' : undefined} ta={IS_HANDHELD_DEVICE ? 'center' : undefined}>
          <Text>{t('deleteAccountConfirmationStart')}</Text>
          {hasCredit(accountInfo) && <Text>{t('deleteAccountConfirmationCredit')}</Text>}
          {hasAppleSubscription(accountInfo) && <Text>{t('deleteAccountConfirmationAppleSubscription')}</Text>}
          <Text>{t('deleteAccountConfirmationEnd')}</Text>
        </Stack>
        <DeleteAccount onClick={deleteAccount} />
      </Stack>
    </ConfirmationDialog>
    <ConfirmationDialog opened={confirmLogOut} onClose={closeLogOutConfirm} title={t('logOut')} drawerCloseButton={false}>
      <Stack h='100%' justify='space-between'>
        <Text><Trans i18nKey='logOutPrompt' context={osStatus.osVpnStatus === NEVPNStatus.Connected ? 'connected' : 'disconnected'} components={{ b: <b /> }} /></Text>
        <Group justify='flex-end' gap='sm' w='100%' grow={IS_HANDHELD_DEVICE}>
          <Button variant='default' onClick={closeLogOutConfirm}>{t('Cancel')}</Button>
          <Button color='red.7' onClick={logOut}>
            <Group gap={5} ml={0}>
              <IoLogOutOutline size={19} />
              <Text fw={550}>{t('logOut')}</Text>
            </Group>
          </Button>
        </Group>
      </Stack>
    </ConfirmationDialog>
    <Stack align='center' className={classes.container}>
      <AccountStatusCard />
      <AccountNumberSection accountId={accountId} logOut={openLogOutConfirm} />
      <WGConfigurations />
      <DeleteAccount onClick={openDeleteAccount} />
      <MobileLogOut logOut={openLogOutConfirm} />
    </Stack>
  </>;
}

interface AccountStatusProps {
  accountInfo: AccountInfo,
}

function AccountStatusCard() {
  const { appStatus } = useContext(AppContext);
  const { account } = appStatus;

  useReRenderWhenExpired(account);

  if (account === null) return <AccountInfoUnavailable />;

  const accountInfo = account.account_info;
  const creditExpiresAt = accountInfo.top_up?.credit_expires_at;
  const topupExpires = creditExpiresAt !== undefined ? new Date(creditExpiresAt * 1000) : undefined;
  const topUpActive = topupExpires !== undefined && topupExpires.getTime() > new Date().getTime();
  if (accountIsExpired(accountInfo)) {
    return <AccountExpired />
  } else if (isRenewing(accountInfo) && topUpActive) {
    return <AccountPaidUpSubscriptionActive accountInfo={accountInfo} />
  } else if (isRenewing(accountInfo)) {
    return <SubscriptionActive accountInfo={accountInfo} />
  } else if (hasActiveSubscription(accountInfo)) {
    return <SubscriptionPaused accountInfo={accountInfo} />
  }
  const { days: daysLeft } = accountTimeRemaining(accountInfo);
  if (daysLeft < 10)
    return <AccountExpiringSoon accountInfo={accountInfo} />;
  return <AccountPaidUp accountInfo={accountInfo} />
}

function AccountInfoUnavailable() {
  const { t } = useTranslation();
  const {
    osStatus
  } = useContext(AppContext);
  const { internetAvailable } = osStatus;
  return (
    <AccountStatusCardTemplate
      icon={<ThemeIcon c='red.7' variant='transparent'>{internetAvailable ? <BsQuestionSquareFill size={26} /> : <MdOutlineWifiOff size={26} />}</ThemeIcon>}
      heading={t('account-InfoUnavailable')}
      subtitle={<Text size='sm' c='dimmed'>{internetAvailable ? t('pleaseCheckAgain') : t('noInternet')}</Text>}
    />
  );
}

function AccountPaidUpSubscriptionActive({ accountInfo }: AccountStatusProps) {
  const { t } = useTranslation();
  const topupExpires = new Date(accountInfo.top_up!.credit_expires_at * 1000);
  const endDate = topupExpires.toLocaleDateString();
  return (
    <AccountStatusCardTemplate
      icon={<PaidUpSubscriptionActive />}
      heading={t('account-SubscriptionActive')}
      subtitle={<Text size='sm' c='dimmed'>{t('account-SubscriptionWillStart', { endDate })}</Text>}
    />
  );
}

function SubscriptionActive({ accountInfo }: AccountStatusProps) {
  const { t } = useTranslation();
  const accountPaidUntil = paidUntil(accountInfo);
  const { days: daysLeft } = accountTimeRemaining(accountInfo);
  const tOptions = {
    count: daysLeft,
    endDate: accountPaidUntil!.toLocaleDateString(),
    context: `${daysLeft}`
  };
  return (
    <AccountStatusCardTemplate
      icon={<SubscriptionActiveBadge />}
      heading={t('account-SubscriptionActive')}
      subtitle={<Text size='sm' c='dimmed'>{t('account-SubscriptionRenewsOn', tOptions)}</Text>}
    />
  );
}

function SubscriptionPaused({ accountInfo }: AccountStatusProps) {
  const { t } = useTranslation();
  const accountPaidUntil = paidUntil(accountInfo);
  const endDate = accountPaidUntil!.toLocaleDateString();
  return (
    <AccountStatusCardTemplate
      icon={<SubscriptionPausedBadge />}
      heading={t('account-SubscriptionPaused')}
      subtitle={<Text size='sm' c='dimmed'>{t('account-SubscriptionAutoRenewSubtitle', { endDate })}</Text>}
    />
  );
}

function AccountExpired() {
  const { t } = useTranslation();
  return (
    <AccountStatusCardTemplate
      icon={<AccountExpiredBadge />}
      heading={t('account-Expired')}
      subtitle={<Text size='sm' c='dimmed'>{t('continueUsingObscura')}</Text>}
    />
  );
}

function AccountPaidUp({ accountInfo }: AccountStatusProps) {
  const { t } = useTranslation();
  const accountPaidUntil = paidUntil(accountInfo);
  const { days: daysLeft } = accountTimeRemaining(accountInfo);
  const tOptions = {
    count: daysLeft,
    endDate: accountPaidUntil!.toLocaleDateString(),
    context: `${daysLeft}`
  };
  return (
    <AccountStatusCardTemplate
      icon={<PaidUpBadge />}
      heading={t('account-PaidUp')}
      subtitle={<Text size='sm' c='dimmed'><Trans i18nKey='account-ExpiresOn' values={tOptions} components={[<Text component='span' display='inline-block' fw='bold' />]} /></Text>}
    />
  );
}

function AccountExpiringSoon({ accountInfo }: AccountStatusProps) {
  const { t } = useTranslation();
  const accountPaidUntil = paidUntil(accountInfo);
  const timeRemaining = accountTimeRemaining(accountInfo);
  const { days, hours, minutes } = timeRemaining;

  const { heading, subtitle } = useExpiryMessages(
    days,
    hours,
    minutes,
    accountPaidUntil!
  );

  return (
    <AccountStatusCardTemplate
      icon={days < 5 ? <PaidUpExpiringVerySoonBadge /> : <PaidUpExpiringSoonBadge />}
      heading={heading}
      subtitle={
        <Stack gap={0}>
          <Text size='sm'>{subtitle}</Text>
          <Text size='sm' c='dimmed'>{t('continueUsingObscura')}</Text>
        </Stack>
      }
    />
  );
}

interface ExpiryMessages {
  heading: string;
  subtitle: string;
}

function useExpiryMessages(
  days: number,
  hours: number,
  minutes: number,
  accountPaidUntil: Date
): ExpiryMessages {
  const { t } = useTranslation();
  let heading: string;
  let subtitle: string;
  let expiryInfo: { count: number; endDate: string };

  if (days === 0 && hours === 0) {
    // Show minutes when less than 1 hour remains
    expiryInfo = {
      count: minutes,
      endDate: accountPaidUntil.toLocaleDateString(),
    };
    heading = t('account-MinutesUntilExpiry', expiryInfo);
    subtitle = t('account-ExpiresInMinutes', expiryInfo);
  } else if (days === 0) {
    // Show hours when less than 1 day but at least 1 hour remains
    expiryInfo = {
      count: hours,
      endDate: accountPaidUntil.toLocaleDateString(),
    };
    heading = t('account-HoursUntilExpiry', expiryInfo);
    subtitle = t('account-ExpiresInHours', expiryInfo);
  } else {
    // Show days when 1 or more days remain
    expiryInfo = {
      count: days,
      endDate: accountPaidUntil.toLocaleDateString(),
    };
    const verySoon = days < 5;
    heading = t('account-DaysUntilExpiry', expiryInfo);
    subtitle = t(verySoon ? 'account-ExpiresVerySoon' : 'account-ExpiresSoon', expiryInfo);
  }

  return { heading, subtitle };
}

interface AccountStatusCardTemplateProps {
  icon: React.ReactNode,
  heading: string,
  subtitle: React.ReactNode,
}

function AccountStatusCardTemplate({
  icon,
  heading,
  subtitle
}: AccountStatusCardTemplateProps) {
  const { t } = useTranslation();
  const { appStatus } = useContext(AppContext);
  return (
    <Paper w='100%' p='md' radius='md' shadow='sm' className={commonClasses.elevatedSurface}>
      <Group grow preventGrowOverflow={false}>
        <Box maw='min-content'>
          {icon}
        </Box>
        <Box className={classes.accountStatusCardBox}>
          <Text fw={500}>{heading}</Text>
          {subtitle}
        </Box>
        <Stack maw='min-content' visibleFrom='xs'>
          <Group justify='right'>
            <AccountRefreshButton smallerSize />
          </Group>
          <ManagePaymentsButton />
        </Stack>
      </Group>
      <Group grow mt='xs' hiddenFrom='xs'>
        <Group justify='center'>
          <AccountRefreshButton />
        </Group>
        <ManagePaymentsButton mobile />
      </Group>
    </Paper>
  );
}

function AccountRefreshButton({ smallerSize = false }: { smallerSize?: boolean }) {
  const { t } = useTranslation();
  const { pollAccount, accountLoading } = useContext(AppContext);
  const theme = useMantineTheme();

  const onRefresh = async () => {
    try {
      await pollAccount();
    } catch (e) {
      const error = normalizeError(e);
      const message = error instanceof commands.CommandError
        ? fmtErrorI18n(t, error) : error.message;
      notifications.show({
        title: t('Account Error'),
        message: message,
        color: 'red',
      });
    }
  }

  const smallerRefresh = smallerSize && !IS_HANDHELD_DEVICE;

  return (
    <Anchor onClick={onRefresh} fw={550} c={theme.primaryColor}
      size={smallerRefresh ? 'sm' : 'md'}>
      {accountLoading ? (
        <Center w={{ base: undefined, xs: 60 }}>
          {/* set height of loader to avoid layout shifts */}
          <Loader h='1.25rem' size={smallerRefresh ? 'xs' : 'sm'} />
        </Center>
      ) :
        <Group gap={5}><FaRotateRight size={smallerRefresh ? 11 : 13} /> {t('Refresh')}</Group>
      }
    </Anchor>
  );
}

function WGConfigurations() {
  const { t } = useTranslation();
  const theme = useMantineTheme();
  const { appStatus } = useContext(AppContext);
  return <>
    <Stack align='start' w='100%' p='md' style={{ borderRadius: theme.radius.md, boxShadow: theme.shadows.sm }} className={commonClasses.elevatedSurface}>
      <Group w='100%' justify='space-between'>
        <Text fw={500}>{t('WGConfigs')}</Text>
        <ButtonLink href={ObscuraAccount.tunnelsUrl(appStatus.accountId)}>{t('Manage Configurations')}</ButtonLink>
      </Group>
    </Stack>
  </>
}

function DeleteAccount({ onClick }: { onClick: () => void }) {
  const { t } = useTranslation();
  return <Group align='start' w='100%'>
    <Button onClick={onClick} variant='light' color='red.7' w={{ base: '100%', xs: 'auto' }}>
      <Group gap={5} ml={0}>
        <IoNuclear size={19} />
        <Text fw={550}>{t('deleteAccount')}</Text>
      </Group>
    </Button>
  </Group>;
}

function ManagePaymentsButton({ mobile = false }: { mobile?: boolean }) {
  const { t } = useTranslation();
  const { appStatus } = useContext(AppContext);
  const [paymentSheetOpened, { open: openPaymentSheet, close: closePaymentSheet }] = useDisclosure(false);

  if (IS_HANDHELD_DEVICE) {
    return <>
      <PaymentManagementSheet opened={paymentSheetOpened} onClose={closePaymentSheet} />
      <Button onClick={openPaymentSheet} w={{ base: '100%', xs: 'auto' }}>
        {mobile ? t('Manage') : t('Manage Payments')}
      </Button>
    </>;
  }

  return (
    <ButtonLink
      href={ObscuraAccount.payUrl(appStatus.accountId)}
    >{mobile ? t('Manage') : t('Manage Payments')}</ButtonLink>
  );
}

function MobileLogOut({ logOut }: { logOut: () => void }) {
  const { t } = useTranslation();
  return <>
    <Button className={commonClasses.mobileOnly} fw='bolder' onClick={logOut} color='red.7' variant='outline' w='100%'>
      <Group gap={5} w='100%'>
        <IoLogOutOutline size={19} />
        <Text fw={550}>{t('logOut')}</Text>
      </Group>
    </Button>
  </>;
}

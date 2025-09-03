import { Anchor, Box, Button, Center, Code, Group, Loader, Paper, Stack, Text, ThemeIcon, useMantineTheme } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import React, { useContext, useEffect } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { BsQuestionSquareFill } from 'react-icons/bs';
import { FaRotateRight } from 'react-icons/fa6';
import { IoLogOutOutline } from 'react-icons/io5';
import { MdOutlineWifiOff } from 'react-icons/md';
import * as commands from '../bridge/commands';
import { IS_HANDHELD_DEVICE } from '../bridge/SystemProvider';
import * as ObscuraAccount from '../common/accountUtils';
import { AccountInfo, accountIsExpired, hasActiveSubscription, isRenewing, paidUntil, paidUntilDays, useReRenderWhenExpired } from '../common/api';
import { AppContext } from '../common/appContext';
import commonClasses from '../common/common.module.css';
import { normalizeError } from '../common/utils';
import { AccountNumberSection } from '../components/AccountNumberSection';
import { ButtonLink } from '../components/ButtonLink';
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
  const { appStatus, pollAccount } = useContext(AppContext);

  useEffect(() => {
    // Ensure account info is up-to-date when the user is viewing the account page.
    void pollAccount();
  }, []);

  const logOut = async () => {
    try {
      await commands.logout();
    } catch (e) {
      const error = normalizeError(e);
      notifications.show({ title: t('logOutFailed'), message: <Text>{t('pleaseReportError')}<br /><Code>{error.message}</Code></Text> });
    }
  }

  // vpnStatus is used because accountInfo will be null if pollAccount fails
  const accountId = appStatus.accountId;
  return (
    <Stack align='center' mt='sm' className={classes.container}>
      <AccountStatusCard />
      <AccountNumberSection accountId={accountId} logOut={logOut} />
      <WGConfigurations />
      <MobileLogOut logOut={logOut} />
    </Stack>
  );
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
  const expiryD = paidUntilDays(accountInfo);
  if (expiryD < 10)
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
  const daysLeft = paidUntilDays(accountInfo);
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
  const daysLeft = paidUntilDays(accountInfo);
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
  const expiryInfo = {
    count: paidUntilDays(accountInfo),
    endDate: accountPaidUntil!.toLocaleDateString(),
  };
  const verySoon = expiryInfo.count < 5;
  const i18nKey = verySoon ? 'account-ExpiresVerySoon' : 'account-ExpiresSoon';
  return (
    <AccountStatusCardTemplate
      icon={expiryInfo.count < 5 ? <PaidUpExpiringVerySoonBadge /> : <PaidUpExpiringSoonBadge />}
      heading={t('account-DaysUntilExpiry', expiryInfo)}
      subtitle={
        <Stack gap={0}>
          <Text size='sm'>{t(i18nKey, expiryInfo)}</Text>
          <Text size='sm' c='dimmed'>{t('continueUsingObscura')}</Text>
        </Stack>
      }
    />
  );
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
          <ButtonLink text={t('Manage Payments')} href={ObscuraAccount.payUrl(appStatus.accountId)} />
        </Stack>
      </Group>
      <Group grow mt='xs' hiddenFrom='xs'>
        <Group justify='center'>
          <AccountRefreshButton />
        </Group>
        <ButtonLink text={t('Manage')} href={ObscuraAccount.payUrl(appStatus.accountId)} />
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
        <ButtonLink text={t('Manage Configurations')} href={ObscuraAccount.tunnelsUrl(appStatus.accountId)} />
      </Group>
    </Stack>
  </>
}

function MobileLogOut({ logOut }: { logOut: () => Promise<void> }) {
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

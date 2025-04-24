import { Anchor, Box, Center, Group, Loader, Paper, Stack, Text, ThemeIcon, useComputedColorScheme, useMantineTheme } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import React, { useContext, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { BsQuestionSquareFill } from 'react-icons/bs';
import { FaRotateRight } from 'react-icons/fa6';
import { MdOutlineWifiOff } from 'react-icons/md';
import * as commands from '../bridge/commands';
import * as ObscuraAccount from '../common/accountUtils';
import { AccountInfo, accountIsExpired, getActiveSubscription, isRenewing, paidUntil, paidUntilDays, useReRenderWhenExpired } from '../common/api';
import { AppContext } from '../common/appContext';
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

export default function Account() {
    const { t } = useTranslation();
    const theme = useMantineTheme();
    const { appStatus, pollAccount } = useContext(AppContext);

    useEffect(() => {
        // Ensure account info is up-to-date when the user is viewing the account page.
        void pollAccount();
    }, []);

    // vpnStatus is used because accountInfo will be null if pollAccount fails
    const accountId = appStatus.accountId;
    const colorScheme = useComputedColorScheme();
    return (
        <Stack align='center' p={20} gap='xl' mt='sm'>
            <AccountStatusCard />
            <AccountNumberSection accountId={accountId} />
            <Stack align='start' w='90%' p='md' style={{ borderRadius: theme.radius.md, boxShadow: theme.shadows.sm }} bg={colorScheme === 'light' ? 'gray.1' : 'dark.6'}>
                <Group w='100%' justify='space-between'>
                    <Text fw={500}>{t('WGConfigs')}</Text>
                    <ButtonLink text={t('Manage Configurations')} href={ObscuraAccount.tunnelsUrl(appStatus.accountId)} />
                </Group>
            </Stack>
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
    } else if (getActiveSubscription(accountInfo)) {
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
            shaveOff={100}
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
            subtitle={<Text size='sm' c='dimmed'>{t('account-ExpiresOn', tOptions)}</Text>}
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
    shaveOff?: number
}

function AccountStatusCardTemplate({
    icon,
    heading,
    subtitle,
    shaveOff = 60
}: AccountStatusCardTemplateProps) {
    const colorScheme = useComputedColorScheme();
    const { appStatus } = useContext(AppContext);
    const { t } = useTranslation();
    return (
        <Paper w='90%' p='md' radius='md' bg={colorScheme === 'light' ? 'gray.1' : 'dark.6'} shadow='sm'>
            <Group>
                {icon}
                <Box w={`calc(100% - ${shaveOff}px)`}>
                    <Group justify='space-between'>
                        <Text fw={500}>{heading}</Text>
                        <Group>
                          <CheckAgain />
                          <ButtonLink text={t('Manage Payments')} href={ObscuraAccount.payUrl(appStatus.accountId)} />
                        </Group>
                    </Group>
                    {subtitle}
                </Box>
            </Group>
        </Paper>
    );
}

function CheckAgain() {
    const { t } = useTranslation();
    const { pollAccount, accountLoading } = useContext(AppContext);
    const theme = useMantineTheme();

    return (
        <Group>
            <Anchor onClick={async () => {
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
            }} fw={550} c={theme.primaryColor}>{(accountLoading) ? <Center w={100}><Loader size='sm' /></Center> : <><FaRotateRight size={13} /> {t('Recheck')}</>}</Anchor>
        </Group>
    );
}

import { Anchor, Box, Button, Center, Code, Group, Loader, Paper, Stack, Text, ThemeIcon, useComputedColorScheme, useMantineTheme } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import React, { useContext, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BsQuestionSquareFill } from 'react-icons/bs';
import { FaExternalLinkSquareAlt } from 'react-icons/fa';
import { IoIosRefresh } from 'react-icons/io';
import { IoLogOutOutline } from 'react-icons/io5';
import { MdOutlineWifiOff } from 'react-icons/md';
import * as commands from '../bridge/commands';
import * as ObscuraAccount from '../common/accountUtils';
import { AccountInfo, accountIsExpired, getActiveSubscription, isRenewing, paidUntil, paidUntilDays } from '../common/api';
import { AppContext } from '../common/appContext';
import { fmtErrorI18n } from '../common/danger';
import { normalizeError } from '../common/utils';
import { AccountNumberDisplay } from '../components/AccountNumberDisplay';
import AccountExpiredBadge from '../res/account-expired.svg?react';
import PaidUpExpiringSoonBadge from '../res/paid-up-expiring-soon.svg?react';
import PaidUpExpiringVerySoonBadge from '../res/paid-up-expiring-very-soon.svg?react';
import PaidUpSubscriptionActive from '../res/paid-up-subscription-active.svg?react';
import PaidUpBadge from '../res/paid-up.svg?react';
import SubscriptionActiveBadge from '../res/subscription-active.svg?react';
import SubscriptionPausedBadge from '../res/subscription-paused.svg?react';

export default function Account() {
    const { t } = useTranslation();
    const theme = useMantineTheme();
    const { appStatus, accountInfo, pollAccount } = useContext(AppContext);

    useEffect(() => {
        // Ensure account info is up-to-date when the user is viewing the account page.
        void pollAccount();
    }, []);

    // vpnStatus is used because accountInfo will be null if pollAccount fails
    const accountId = appStatus.accountId;

    const logOut = async () => {
        try {
            await commands.logout();
        } catch (e) {
            const error = normalizeError(e);
            notifications.show({ title: t('logOutFailed'), message: <Text>{t('pleaseReportError')}<br /><Code>{error.message}</Code></Text> });
        }
    }

    return (
        <Stack align='center' p={20} gap='xl' mt='sm'>
            <Stack w='100%' align='center'>
                <AccountStatusCard accountInfo={accountInfo} />
                <Group w='90%' justify='right'>
                    <ManageSubscriptionLink accountId={appStatus.accountId} />
                </Group>
            </Stack>
            {accountId && <Box w='90%'>
                <AccountNumberDisplay accountId={accountId} />
            </Box>}
            <Box w='90%'>
                <Button fw='bolder' onClick={logOut} {...theme.other.buttonDisconnectProps}>
                    <Group gap={5}>
                        <IoLogOutOutline size={19} />
                        <Text fw={550}>{t('logOut')}</Text>
                    </Group>
                </Button>
            </Box>
        </Stack >
    );
}

interface ManagePaymentLinkProps {
    accountId: ObscuraAccount.AccountId,
}

function ManageSubscriptionLink({ accountId }: ManagePaymentLinkProps) {
    const { t } = useTranslation();
    // TODO: Call the API to get the Stripe URL and go directly there.
    return (
        <Button component='a' href={ObscuraAccount.payUrl(accountId)} size='sm'>
            <span>{t('Manage Payments')} <FaExternalLinkSquareAlt size={11} /></span>
        </Button>
    );
}

interface AccountStatusProps {
    accountInfo: AccountInfo,
}

function AccountStatusCard({
    accountInfo,
}: { accountInfo: AccountInfo | null }) {
    if (accountInfo === null) return <AccountInfoUnavailable />;

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
                    <Text>{t(i18nKey, expiryInfo)}</Text>
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
    return (
        <Paper w='90%' p='md' radius='md' bg={colorScheme === 'light' ? 'gray.1' : 'dark.6'}>
            <Group>
                {icon}
                <Box w={`calc(100% - ${shaveOff}px)`}>
                    <Group justify='space-between'>
                        <Text fw={500}>{heading}</Text>
                        <CheckAgain />
                    </Group>
                    {subtitle}
                </Box>
            </Group>
        </Paper>
    );
}

function CheckAgain() {
    const { t } = useTranslation();
    const { pollAccount } = useContext(AppContext);
    const [accountRefreshing, setAccountRefreshing] = useState(false);

    return (
        <Group>
            <Anchor onClick={async () => {
                if (!accountRefreshing) {
                    try {
                        setAccountRefreshing(true);
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
                    } finally {
                        setAccountRefreshing(false);
                    }
                }
            }} c='gray.6'>{accountRefreshing ? <Center w={100}><Loader size='sm' /></Center> : <><IoIosRefresh size={13} /> <u>{t('Recheck')}</u></>}</Anchor>
        </Group>
    );
}

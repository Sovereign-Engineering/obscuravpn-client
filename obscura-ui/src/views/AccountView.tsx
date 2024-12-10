import { Anchor, Box, Button, Code, Group, Loader, Paper, Stack, Text, ThemeIcon, useComputedColorScheme, useMantineTheme } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import React, { useContext, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BsCheckCircleFill, BsExclamationTriangleFill } from 'react-icons/bs';
import { IoLogOutOutline } from 'react-icons/io5';
import { MdAutorenew } from 'react-icons/md';
import * as commands from '../bridge/commands';
import * as ObscuraAccount from '../common/accountUtils';
import { AccountInfo, getActiveSubscription, isRenewing, paidUntil, paidUntilDays } from '../common/api';
import { AppContext } from '../common/appContext';
import { normalizeError } from '../common/utils';
import { AccountNumberDisplay } from '../components/AccountNumberDisplay';

export default function Account() {
    const { t } = useTranslation();
    const theme = useMantineTheme();
    let { appStatus, accountInfo, pollAccount } = useContext(AppContext);

    useEffect(() => {
        // Ensure account info is up-to-date when the user is viewing the account page.
        void pollAccount();
    }, []);

    // vpnStatus is used because accountInfo will be null if pollAccount fails
    let accountId = appStatus.accountId;

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
            {accountInfo && <AccountStatusCard accountInfo={accountInfo} />}
            {accountId && <Box w='90%'>
                <AccountNumberDisplay accountId={accountId} />
            </Box>}
            <FooterSection accountInfo={accountInfo} />
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

interface AccountStatusProps {
    accountInfo: AccountInfo,
}

function AccountStatusCard({
    accountInfo,
}: AccountStatusProps) {
    if (!accountInfo.active) {
        return <AccountExpired accountInfo={accountInfo} />
    } else if (isRenewing(accountInfo)) {
        return <AutoRenewalActive accountInfo={accountInfo} />
    } else if (getActiveSubscription(accountInfo)) {
        return <AutoRenewalPrompt accountInfo={accountInfo} />
    }

    let expiryD = paidUntilDays(accountInfo);
    if (expiryD < 29)
        return <AccountExpiringSoon accountInfo={accountInfo} />
    return <FundYourAccount accountInfo={accountInfo} />;
}

function AutoRenewalActive({ accountInfo }: AccountStatusProps) {
    const { t } = useTranslation();
    return (
        <AccountStatusCardTemplate
            icon={<ThemeIcon c='green' variant='transparent'><BsCheckCircleFill size={20} /></ThemeIcon>}
            heading={<Text fw={500}>{t('account-SubscriptionActive')}</Text>}
            subtitle={t('account-GoToPayment')}
            anchor={<ManageSubscriptionLink accountId={accountInfo.id} />}
        />
    );
}

interface ManageSubscriptionLinkProps {
    accountId: ObscuraAccount.AccountId,
}

function ManageSubscriptionLink({ accountId }: ManageSubscriptionLinkProps) {
    const { t } = useTranslation();
    // TODO: Call the API to get the Stripe URL and go directly there.
    return <Anchor href={ObscuraAccount.subscriptionUrl(accountId)} size='sm'>
        {t('Manage subscription')}
    </Anchor>
}

function AutoRenewalPrompt({ accountInfo }: AccountStatusProps) {
    const { t } = useTranslation();
    const daysLeft = paidUntilDays(accountInfo);
    return (
        <AccountStatusCardTemplate showRecheck
            icon={<ThemeIcon c='gray' variant='transparent'><BsExclamationTriangleFill size={20} /></ThemeIcon>}
            heading={
                <Group justify='space-between'>
                    <Text fw={500}>{t('account-SubscriptionTurnOnRenewal')}</Text>
                    <Text size='xs' fw={600}>{t('account-DaysLeft', { daysLeft })}</Text>
                </Group>
            }
            subtitle={t('account-SubscriptionAutoRenewSubtitle')}
            anchor={<ManageSubscriptionLink accountId={accountInfo.id} />}
        />
    );
}

function AccountExpired({ accountInfo }: AccountStatusProps) {
    const { t } = useTranslation();
    return (
        <AccountStatusCardTemplate showRecheck
            heading={t('account-Expired')}
            subtitle={t('account-GoToPayment')}
            icon={<ThemeIcon c='red.7' variant='transparent'><BsExclamationTriangleFill size={20} /></ThemeIcon>}
            anchor={<Anchor href={ObscuraAccount.payUrl(accountInfo.id)} size='sm'>
                {t('Payments')}
            </Anchor>}
        />
    );
}

function AccountExpiringSoon({ accountInfo }: AccountStatusProps) {
    const { t } = useTranslation();
    const daysLeft = paidUntilDays(accountInfo);
    return (
        <AccountStatusCardTemplate showRecheck
            icon={<ThemeIcon c='yellow.5' variant='transparent'><BsExclamationTriangleFill size={20} /></ThemeIcon>}
            heading={
                <Group justify='space-between'>
                    <Text fw={500}>{t('account-ExpiresSoon')}</Text>
                    <Text size='xs' fw={600}>{t('account-DaysLeft', { daysLeft })}</Text>
                </Group>
            }
            subtitle={t('account-GoToPayment')}
            anchor={<Anchor href={ObscuraAccount.payUrl(accountInfo.id)} size='sm'>
                {t('Payments')}
            </Anchor>}
        />
    );
}

interface AccountStatusCardTemplateProps {
    icon: React.ReactNode,
    heading: React.ReactNode,
    subtitle: string,
    anchor: React.ReactNode,
    showRecheck?: boolean,
}

function AccountStatusCardTemplate({
    icon,
    heading,
    subtitle,
    anchor,
    showRecheck = false,
}: AccountStatusCardTemplateProps) {
    const colorScheme = useComputedColorScheme();
    return <Paper w='90%' p='md' radius='md' bg={colorScheme === 'light' ? 'gray.2' : 'dark.5'}>
        <Group>
            {icon}
            <Box w='calc(100% - 50px)'>
                {heading}
                <Text size='sm' c='dimmed'>{subtitle}</Text>
                <Group mt={5} h={30}>
                    {anchor}
                    {showRecheck && <RecheckButton />}
                </Group>
            </Box>
        </Group>
    </Paper>
}

function FundYourAccount({ accountInfo }: AccountStatusProps) {
    const { t } = useTranslation();
    return (
        <Box w='90%'>
            <Button component='a' href={ObscuraAccount.payUrl(accountInfo.id)}>
                {t('fundYourAccount')}
            </Button>
        </Box>
    )
}

function RecheckButton() {
    const { t } = useTranslation();
    const { pollAccount } = useContext(AppContext);
    const [accountRefreshing, setAccountRefreshing] = useState(false);

    return (
        <Button w={100} disabled={accountRefreshing} onClick={async () => {
            try {
                setAccountRefreshing(true);
                await pollAccount();
            } catch (e) {
                const error = normalizeError(e);
                notifications.show({
                    title: t('Account Error'),
                    message: t(error.message),
                    color: "red",
                });
            } finally {
                setAccountRefreshing(false);
            }
        }} variant='subtle' c='teal'>{accountRefreshing ? <Loader size='sm' /> : t('Recheck')}</Button>
    );
}

interface FooterSectionProps {
    accountInfo: AccountInfo | null,
}

function FooterSection({ accountInfo }: FooterSectionProps) {
    const { t } = useTranslation();
    if (accountInfo === null) {
        return (
            <Stack w='90%'>
                <Text fw={500} c='red'>{t('account-InfoUnavailable')}</Text>
                <RecheckButton />
            </Stack>
        );
    }

    if (!accountInfo.active) {
        // if an account is inactive/expired, AccountStatusCard will be visible
        return;
    }

    let expiryInfo = {
        daysLeft: paidUntilDays(accountInfo),
        endDate: paidUntil(accountInfo)?.toLocaleDateString(),
    };

    if (!expiryInfo.endDate) {
        console.warn("Active account without subscription or top-up. We don't know why it is active.")
        return (
            <Box w='90%'>
                <Text mb='xs' fw={500}>{t('Expiration')}</Text>
                <Text size='sm'>
                    {t('Account is active.')}
                </Text>
            </Box>
        );
    }

    let subscription = getActiveSubscription(accountInfo);
    if (subscription) {
        return (
            <Box w='90%'>
                <Group mb='xs' gap={5}>
                    <MdAutorenew size={20} />
                    <Text fw={500}>{t('Subscription')}</Text>
                </Group>
                <Text size='sm'>
                    {
                        isRenewing(accountInfo) ?
                            t('account-SubscriptionRenewsOn', expiryInfo) :
                            t('account-SubscriptionExpiresOn', expiryInfo)
                    }
                </Text>
            </Box>
        );
    }

    return (
        <Box w='90%'>
            <Text mb='xs' fw={500}>{t('Expiration')}</Text>
            <Text size='sm'>
                {t('account-ExpiresOn', expiryInfo)}
            </Text>
        </Box>
    );
}

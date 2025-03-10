import { ActionIcon, CopyButton, Group, Text, useComputedColorScheme, useMantineTheme } from '@mantine/core';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BsClipboardFill } from 'react-icons/bs';
import * as ObscuraAccount from '../common/accountUtils';
import Eye from '../res/eye.fill.svg?react';
import EyeSlash from '../res/eye.slash.fill.svg?react';
import PersonBadgeKey from '../res/person.badge.key.svg?react';

export function AccountNumberDisplay({ accountId }: { accountId: ObscuraAccount.AccountId }) {
    const theme = useMantineTheme();
    const { t } = useTranslation();
    const [showAccountNumber, setShowAccountNumber] = useState(false);
    const colorScheme = useComputedColorScheme();
    const EyeIcon = showAccountNumber ? EyeSlash : Eye;
    const primaryColorResolved = theme.variantColorResolver({color: theme.primaryColor, theme, variant: 'subtle'}).color;
    return (
        <>
            <Group mb='xs' gap={5}>
                <PersonBadgeKey fill={colorScheme === 'light' ? 'black' : 'white'} width='1em' height='1em' />
                <Text fw={500}>Account Number</Text>
                <ActionIcon variant='subtle' title={showAccountNumber ? t('hide account number') : t('show account number')} onClick={() => setShowAccountNumber(!showAccountNumber)}>
                    {<EyeIcon fill={primaryColorResolved} width='1em' height='1em' />}
                </ActionIcon>
                <CopyButton value={ObscuraAccount.accountIdToString(accountId)}>
                    {({ copied, copy }) => (
                        <ActionIcon c={copied ? 'green' : undefined} variant='subtle' title={t('copy account number')} onClick={copy}>
                            <BsClipboardFill size='1em' />
                        </ActionIcon>
                    )}
                </CopyButton>
            </Group>
            <Text ff='monospace'>
                {showAccountNumber
                    ? ObscuraAccount.formatPartialAccountId(ObscuraAccount.accountIdToString(accountId))
                    : 'XXXX - XXXX - XXXX - XXXX - XXXX'}
            </Text>
        </>
    );
}

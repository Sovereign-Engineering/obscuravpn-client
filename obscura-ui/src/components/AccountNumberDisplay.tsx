import { ActionIcon, CopyButton, Group, Text, useComputedColorScheme } from '@mantine/core';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BsClipboardFill } from 'react-icons/bs';
import { FaEye, FaEyeSlash } from 'react-icons/fa';
import * as ObscuraAccount from '../common/accountUtils';
import PersonBadgeKey from '../res/person.badge.key.svg?react';

export function AccountNumberDisplay({ accountId }: { accountId: ObscuraAccount.AccountId }) {
    const { t } = useTranslation();
    const [showAccountNumber, setShowAccountNumber] = useState(false);
    const colorScheme = useComputedColorScheme();
    return (
        <>
            <Group mb='xs' gap={5}>
                <PersonBadgeKey fill={colorScheme === 'light' ? 'black' : 'white'} width='1em' height='1em' />
                <Text fw={500}>Account Number</Text>
                <ActionIcon variant='subtle' title={showAccountNumber ? t('hide account number') : t('show account number')} onClick={() => setShowAccountNumber(!showAccountNumber)}>
                    {showAccountNumber ? <FaEyeSlash size='1em' /> : <FaEye size='1em' />}
                </ActionIcon>
                <CopyButton value={accountId.toString()}>
                    {({ copied, copy }) => (
                        <ActionIcon c={copied ? 'green' : 'orange'} variant='subtle' title={t('copy account number')} onClick={copy}>
                            <BsClipboardFill size='1em' />
                        </ActionIcon>
                    )}
                </CopyButton>

            </Group>
            <Text ff='monospace'>
                {showAccountNumber
                    ? ObscuraAccount.formatPartialAccountId(accountId.toString())
                    : 'XXXX - XXXX - XXXX - XXXX - XXXX'}
            </Text>
        </>
    );
}

import { ActionIcon, Button, Code, CopyButton, Group, Stack, Text, useComputedColorScheme, useMantineTheme } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BsClipboardFill } from 'react-icons/bs';
import { IoLogOutOutline } from 'react-icons/io5';
import * as commands from '../bridge/commands';
import * as ObscuraAccount from '../common/accountUtils';
import { normalizeError, usePrimaryColorResolved } from '../common/utils';
import Eye from '../res/eye.fill.svg?react';
import EyeSlash from '../res/eye.slash.fill.svg?react';
import PersonBadgeKey from '../res/person.badge.key.svg?react';

export function AccountNumberSection({ accountId }: { accountId: ObscuraAccount.AccountId }) {
  const { t } = useTranslation();
  const colorScheme = useComputedColorScheme();
  const theme = useMantineTheme();
  const primaryColorResolved = usePrimaryColorResolved();
  const [showAccountNumber, setShowAccountNumber] = useState(false);
  const EyeIcon = showAccountNumber ? EyeSlash : Eye;

  const logOut = async () => {
    try {
      await commands.logout();
    } catch (e) {
      const error = normalizeError(e);
      notifications.show({ title: t('logOutFailed'), message: <Text>{t('pleaseReportError')}<br /><Code>{error.message}</Code></Text> });
    }
  }

  return (
    <Stack align='start' w='100%' p='md' gap={0} style={{ borderRadius: theme.radius.md, boxShadow: theme.shadows.sm }} bg={colorScheme === 'light' ? 'gray.1' : 'dark.6'}>
      <Group w='100%' justify='space-between'>
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
        <Button fw='bolder' onClick={logOut} c='red.7' variant='subtle'>
          <Group gap={5}>
            <IoLogOutOutline size={19} />
            <Text fw={550}>{t('logOut')}</Text>
          </Group>
        </Button>
      </Group>
      <Text ff='monospace'>
        {showAccountNumber
          ? ObscuraAccount.formatPartialAccountId(ObscuraAccount.accountIdToString(accountId))
          : 'XXXX - XXXX - XXXX - XXXX - XXXX'}
      </Text>
    </Stack>
  );
}

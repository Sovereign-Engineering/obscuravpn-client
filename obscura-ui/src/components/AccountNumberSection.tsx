import { ActionIcon, Button, Code, CopyButton, Group, Stack, Text, useComputedColorScheme, useMantineTheme } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BsClipboardFill } from 'react-icons/bs';
import { FaCopy } from 'react-icons/fa6';
import { IoLogOutOutline } from 'react-icons/io5';
import * as commands from '../bridge/commands';
import * as ObscuraAccount from '../common/accountUtils';
import commonClasses from '../common/common.module.css';
import { normalizeError, usePrimaryColorResolved } from '../common/utils';
import Eye from '../res/eye.fill.svg?react';
import EyeSlash from '../res/eye.slash.fill.svg?react';
import PersonBadgeKey from '../res/person.badge.key.svg?react';

export function AccountNumberSection({ accountId, logOut }: { accountId: ObscuraAccount.AccountId, logOut: () => Promise<void> }) {
  const { t } = useTranslation();
  const colorScheme = useComputedColorScheme();
  const theme = useMantineTheme();
  const primaryColorResolved = usePrimaryColorResolved();
  const [showAccountNumber, setShowAccountNumber] = useState(false);
  const EyeIcon = showAccountNumber ? EyeSlash : Eye;

  return (
    <Stack align='start' w='100%' p='md' gap={0} style={{ borderRadius: theme.radius.md, boxShadow: theme.shadows.sm }} bg={colorScheme === 'light' ? 'gray.1' : 'dark.6'}>
      <Group w='100%' justify='space-between'>
        <Group mb='xs' gap={5}>
          <PersonBadgeKey fill={colorScheme === 'light' ? 'black' : 'white'} width='1em' height='1em' />
          <Text fw={500}>Account Number</Text>
          <div className={commonClasses.desktopOnly}>
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
          </div>
        </Group>
        <div className={commonClasses.desktopOnly}>
          <Button fw='bolder' onClick={logOut} color='red.7' variant='subtle'>
            <Group gap={5}>
              <IoLogOutOutline size={19} />
              <Text fw={550}>{t('logOut')}</Text>
            </Group>
          </Button>
        </div>
      </Group>
      <Text ff='monospace'>
        {showAccountNumber
          ? ObscuraAccount.formatPartialAccountId(ObscuraAccount.accountIdToString(accountId))
          : 'XXXX - XXXX - XXXX - XXXX - XXXX'}
      </Text>
      <Group className={commonClasses.mobileOnly} w='100%' mt='xs' grow>
        <CopyButton value={ObscuraAccount.accountIdToString(accountId)}>
          {({ copied, copy }) => (
            <Button c={copied ? 'white' : undefined} bg={copied ? 'teal' : undefined} variant='light' title={t('copy account number')} onClick={copy}>
              <Group gap='xs'>
                <FaCopy />
                {t('Copy')}
              </Group>
            </Button>
          )}
        </CopyButton>
        <Button variant='light' title={showAccountNumber ? t('hide account number') : t('show account number')} onClick={() => setShowAccountNumber(!showAccountNumber)}>
          {/* TODO ICONss */}
          <Group gap='xs'>
            {<EyeIcon fill={primaryColorResolved} width='1em' height='1em' />}
            {t('Reveal')}
          </Group>
        </Button>
      </Group>
    </Stack>
  );
}

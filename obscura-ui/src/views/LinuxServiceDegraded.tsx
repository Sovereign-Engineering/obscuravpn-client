import { Button, Stack, Text, Title } from '@mantine/core';
import { useTranslation } from 'react-i18next';
import * as commands from '../bridge/commands';
import { LinuxDegradation } from '../common/appContext';
import { TranslationKey } from '../translations/i18n';

interface FixOption {
  action: commands.LinuxFixAction;
  labelKey: TranslationKey;
}

interface Degraded {
  titleKey: TranslationKey;
  messageKey: TranslationKey;
  fixes: FixOption[];
}

function describe(degradation: LinuxDegradation): Degraded {
  switch (degradation) {
    case 'stopped':
    case 'failed':
      return {
        titleKey: 'linuxService-stoppedTitle',
        messageKey: 'linuxService-stoppedMessage',
        fixes: [{ action: 'start', labelKey: 'linuxService-startButton' }],
      };
    case 'disabled':
      return {
        titleKey: 'linuxService-disabledTitle',
        messageKey: 'linuxService-disabledMessage',
        fixes: [
          { action: 'enableAndStart', labelKey: 'linuxService-enableAndStartButton' },
          { action: 'start', labelKey: 'linuxService-startOnceButton' },
        ],
      };
    case 'noAccess':
      return {
        titleKey: 'linuxService-noAccessTitle',
        messageKey: 'linuxService-noAccessMessage',
        fixes: [{ action: 'addOperator', labelKey: 'linuxService-authorizeButton' }],
      };
    case 'notInstalled':
      return {
        titleKey: 'linuxService-notInstalledTitle',
        messageKey: 'linuxService-notInstalledMessage',
        fixes: [],
      };
  }
}

export default function LinuxServiceDegraded({ degradation }: { degradation: LinuxDegradation }) {
  const { t } = useTranslation();
  const { showLoadingUI, execute } = commands.useCommand({ command: commands.linuxFix, showNotification: true });
  const { titleKey, messageKey, fixes } = describe(degradation);

  return (
    <Stack align='center' gap='md' maw={420}>
      <Title order={3} ta='center'>{t(titleKey)}</Title>
      <Text c='dimmed' ta='center'>{t(messageKey)}</Text>
      {fixes.map(fix => (
        <Button key={fix.action} loading={showLoadingUI} onClick={() => execute(fix.action)}>
          {t(fix.labelKey)}
        </Button>
      ))}
    </Stack>
  );
}

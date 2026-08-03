import { Button, Stack, Text, Title } from '@mantine/core';
import { useTranslation } from 'react-i18next';
import * as commands from '../bridge/commands';
import { WindowsServiceDegradation } from '../common/appContext';
import { TranslationKey } from '../translations/i18n';

interface FixOption {
  action: commands.WindowsFixAction;
  labelKey: TranslationKey;
}

interface Degraded {
  titleKey: TranslationKey;
  messageKey: TranslationKey;
  fixes: FixOption[];
}

function describe(degradation: WindowsServiceDegradation): Degraded {
  switch (degradation) {
    case 'stopped':
    case 'failed':
      return {
        titleKey: 'windowsService-stoppedTitle',
        messageKey: 'windowsService-stoppedMessage',
        fixes: [{ action: 'start', labelKey: 'windowsService-startButton' }],
      };
    case 'disabled':
      // A disabled Windows service cannot be started without changing its start mode.
      return {
        titleKey: 'windowsService-disabledTitle',
        messageKey: 'windowsService-disabledMessage',
        fixes: [{ action: 'enableAndStart', labelKey: 'windowsService-enableAndStartButton' }],
      };
    case 'notInstalled':
      return {
        titleKey: 'windowsService-notInstalledTitle',
        messageKey: 'windowsService-notInstalledMessage',
        fixes: [],
      };
    case 'packageIdentityMissing':
      return {
        titleKey: 'windowsService-packageIdentityMissingTitle',
        messageKey: 'windowsService-packageIdentityMissingMessage',
        fixes: [],
      };
    case 'other':
      return {
        titleKey: 'windowsService-otherTitle',
        messageKey: 'windowsService-otherMessage',
        fixes: [],
      };
  }
}

export default function WindowsServiceDegraded({ degradation }: { degradation: WindowsServiceDegradation }) {
  const { t } = useTranslation();
  const { showLoadingUI, execute } = commands.useCommand({ command: commands.windowsFix, showNotification: true });
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

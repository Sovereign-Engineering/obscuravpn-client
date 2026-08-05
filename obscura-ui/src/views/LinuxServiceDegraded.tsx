import { Button, Stack, Text, Title } from '@mantine/core';
import { useTranslation } from 'react-i18next';
import * as commands from '../bridge/commands';
import { LinuxServiceDegradation } from '../common/appContext';
import { TranslationKey } from '../translations/i18n';

interface Fix {
  command: () => Promise<void>;
  labelKey: TranslationKey;
}

interface Detail {
  labelKey: TranslationKey;
  value: string;
}

interface Degraded {
  titleKey: TranslationKey;
  messageKey: TranslationKey;
  details?: Detail[];
  fix?: Fix;
}

function describe(degradation: LinuxServiceDegradation): Degraded {
  if (typeof degradation === 'object') {
    const { serviceVersion, appVersion, installedAppVersionDiffers } = degradation.versionMismatch;
    const offerAppRestart = installedAppVersionDiffers !== false;
    return {
      titleKey: 'linuxService-versionMismatchTitle',
      messageKey: offerAppRestart ? 'linuxService-versionMismatchMessage' : 'linuxService-versionMismatchServiceMessage',
      details: [
        { labelKey: 'linuxService-appVersion', value: appVersion },
        { labelKey: 'linuxService-serviceVersion', value: serviceVersion },
      ],
      fix: offerAppRestart
        ? { command: commands.restartApp, labelKey: 'linuxService-restartAppButton' }
        : { command: () => commands.restartService({ enable: false }), labelKey: 'linuxService-restartServiceButton' },
    };
  }
  switch (degradation) {
    case 'unitInactive':
      return {
        titleKey: 'linuxService-unitInactiveTitle',
        messageKey: 'linuxService-unitInactiveMessage',
        fix: { command: () => commands.restartService({ enable: true }), labelKey: 'linuxService-enableAndStartButton' },
      };
    case 'socketPermissionDenied':
      return {
        titleKey: 'linuxService-socketPermissionDeniedTitle',
        messageKey: 'linuxService-socketPermissionDeniedMessage',
        fix: { command: commands.linuxAddOperator, labelKey: 'linuxService-authorizeButton' },
      };
    case 'unitActivating':
      return {
        titleKey: 'linuxService-unitActivatingTitle',
        messageKey: 'linuxService-unitActivatingMessage',
      };
    case 'unitNotInstalled':
      return {
        titleKey: 'linuxService-unitNotInstalledTitle',
        messageKey: 'linuxService-unitNotInstalledMessage',
      };
    case 'unknown':
      return {
        titleKey: 'linuxService-unknownTitle',
        messageKey: 'linuxService-unknownMessage',
      };
  }
}

export default function LinuxServiceDegraded({ degradation }: { degradation: LinuxServiceDegradation }) {
  const { t } = useTranslation();
  const { showLoadingUI, execute } = commands.useCommand({ command: (fix: () => Promise<void>) => fix(), showNotification: true });
  const { titleKey, messageKey, details, fix } = describe(degradation);

  return (
    <Stack align='center' gap='md' maw={420}>
      <Title order={3} ta='center'>{t(titleKey)}</Title>
      <Text c='dimmed' ta='center'>{t(messageKey)}</Text>
      {details !== undefined && (
        <Stack gap={0}>
          {details.map(({ labelKey, value }) => (
            <Text key={labelKey} c='dimmed' size='sm' ta='center'>{t(labelKey)}: {value}</Text>
          ))}
        </Stack>
      )}
      {fix !== undefined && (
        <Button loading={showLoadingUI} onClick={() => execute(fix.command)}>
          {t(fix.labelKey)}
        </Button>
      )}
    </Stack>
  );
}

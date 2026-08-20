import { ActionIcon, Button, Code, CopyButton, Group, Stack, Text, Title } from '@mantine/core';
import { useTranslation } from 'react-i18next';
import { IoCopy } from 'react-icons/io5';
import * as commands from '../bridge/commands';
import { LinuxServiceDegradation } from '../common/appContext';
import { TranslationKey } from '../translations/i18n';

const UNIT_NAME = 'obscura.service';

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
  terminalCommand?: string;
  restartAppAfterwards?: boolean;
}

function describe(degradation: LinuxServiceDegradation): Degraded {
  if (typeof degradation === 'object') {
    if ('socketPermissionDenied' in degradation) {
      const { user } = degradation.socketPermissionDenied;
      return {
        titleKey: 'linuxService-socketPermissionDeniedTitle',
        messageKey: 'linuxService-socketPermissionDeniedMessage',
        fix: { command: commands.linuxAddOperator, labelKey: 'linuxService-authorizeButton' },
        terminalCommand: user === null ? 'sudo obscura add-operator' : `sudo obscura add-operator ${user}`,
        restartAppAfterwards: true,
      };
    }
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
      terminalCommand: offerAppRestart ? undefined : `sudo systemctl restart ${UNIT_NAME}`,
    };
  }
  switch (degradation) {
    case 'unitInactive':
      return {
        titleKey: 'linuxService-unitInactiveTitle',
        messageKey: 'linuxService-unitInactiveMessage',
        fix: { command: () => commands.restartService({ enable: true }), labelKey: 'linuxService-enableAndStartButton' },
        terminalCommand: `sudo systemctl enable --now ${UNIT_NAME}`,
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
  const fixCommand = commands.useCommand({ command: (fix: () => Promise<void>) => fix(), showNotification: true });
  const restartAppCommand = commands.useCommand({ command: commands.restartApp, showNotification: true });
  const { titleKey, messageKey, details, fix, terminalCommand, restartAppAfterwards } = describe(degradation);

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
        <Button loading={fixCommand.showLoadingUI} onClick={() => fixCommand.execute(fix.command)}>
          {t(fix.labelKey)}
        </Button>
      )}
      {terminalCommand !== undefined && (
        <Stack align='center' gap='xs'>
          <Text c='dimmed' size='sm' ta='center'>{t('linuxService-terminalHint')}</Text>
          <Group gap='xs' wrap='nowrap'>
            <Code style={{ whiteSpace: 'pre-wrap', userSelect: 'text', WebkitUserSelect: 'text' }}>{terminalCommand}</Code>
            <CopyButton value={terminalCommand}>
              {({ copied, copy }) => (
                <ActionIcon c={copied ? 'green' : undefined} variant='subtle' title={t('linuxService-copyCommand')} onClick={copy}>
                  <IoCopy size='1em' />
                </ActionIcon>
              )}
            </CopyButton>
          </Group>
        </Stack>
      )}
      {restartAppAfterwards === true && (
        <Stack align='center' gap='xs'>
          <Text c='dimmed' size='sm' ta='center'>{t('linuxService-restartAppAfterwardsHint')}</Text>
          <Button variant='light' loading={restartAppCommand.showLoadingUI} onClick={() => restartAppCommand.execute()}>
            {t('linuxService-restartAppButton')}
          </Button>
        </Stack>
      )}
    </Stack>
  );
}

import { Anchor, Button, Card, Stack, Text, Title, useComputedColorScheme, useMantineTheme } from '@mantine/core';
import { Trans, useTranslation } from 'react-i18next';
import { FaFileAlt } from 'react-icons/fa';
import * as commands from '../bridge/commands';
import { OsStatus } from '../common/appContext';
import { EMAIL } from '../common/links';
import useMailto from '../common/useMailto';

// this component may be used before appContext is created, and thus requires explicitly passing osStatus
export default function LogsCapture({ osStatus }: { osStatus: OsStatus }) {
  const { t } = useTranslation();
  const theme = useMantineTheme();
  const colorScheme = useComputedColorScheme();
  const mailto = useMailto(osStatus);
  const logCaptureSupported = osStatus.logCapture;
  const logCapture = osStatus.logCapture ?? { inProgress: false, logsAvailable: false };
  const handleCommand = commands.useHandleCommand(t);

  return (
    <Card withBorder radius='lg' p='lg' bg={colorScheme === 'dark' ? theme.colors.dark[6] : theme.colors.gray[0]}>
      <Stack gap='md' align="center">
        <Title order={4} c={colorScheme === 'dark' ? theme.colors.gray[3] : theme.colors.gray[7]}>
          {t('havingTrouble')}
        </Title>
        {
          <Text c='gray' component='span'>
            <Trans i18nKey={logCaptureSupported ? 'supportMsgMobile' : 'supportMsg'}
              values={{ email: EMAIL }} components={[<Anchor href={mailto} />]} />
          </Text>
        }
        {
          logCaptureSupported &&
          <Button disabled={logCapture.inProgress} leftSection={<FaFileAlt />} fullWidth onClick={() => handleCommand(commands.captureLogs)}>
            {logCapture.inProgress ? t('capturingLogs') : t('captureLogs')}
          </Button>
        }
        {
          logCapture.logsAvailable && !logCapture.inProgress &&
          <Button disabled={logCapture.inProgress} leftSection={<FaFileAlt />} fullWidth onClick={() => handleCommand(commands.shareLogs)}>
            {t('shareLogs')}
          </Button>
        }
      </Stack>
    </Card>
  );
}

import { Group, Image, Loader, Stack, Text } from '@mantine/core';
import { useThrottledValue } from '@mantine/hooks';
import AppIcon from '../../../apple/client/Assets.xcassets/AppIcon.appiconset/icon_128x128.png';
import { LinuxServiceDegradation, OsStatusWVpnStatus, WindowsServiceDegradation } from '../common/appContext';
import DebugBundle from '../components/DebugBundle';
import ObscuraWordmark from '../components/ObscuraWordmark';
import LinuxServiceDegraded from './LinuxServiceDegraded';
import WindowsServiceDegraded from './WindowsServiceDegraded';
import { useTranslation } from 'react-i18next';

interface SplashScreenProps {
  osStatus: OsStatusWVpnStatus | null;
  linuxDegradation?: LinuxServiceDegradation;
  windowsDegradation?: WindowsServiceDegradation;
}

export default function SplashScreen({ osStatus, linuxDegradation, windowsDegradation }: SplashScreenProps) {
  const { t } = useTranslation();
  const degraded = linuxDegradation !== undefined || windowsDegradation !== undefined;
  // only show the debug bundle while loading after a prolonged period, but immediately when degraded
  const osStatusThrottled = useThrottledValue(osStatus, 5000);
  return (
    <Stack h='100vh' align='center' justify='center' gap='xl'>
      <Image src={AppIcon} w={64} />
      <ObscuraWordmark />
      {linuxDegradation !== undefined && <LinuxServiceDegraded degradation={linuxDegradation} />}
      {windowsDegradation !== undefined && <WindowsServiceDegraded degradation={windowsDegradation} />}
      {!degraded &&
        <Group>
          <Text>{t('splashScreenText')}</Text>
          <Loader size='xl' type='bars' />
        </Group>}
      {osStatus !== null && (degraded || osStatusThrottled !== null) &&
        <DebugBundle osStatus={osStatus} />}
    </Stack>
  );
}

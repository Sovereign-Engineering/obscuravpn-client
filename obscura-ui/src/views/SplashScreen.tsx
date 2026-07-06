import { Group, Image, Loader, Stack, Text } from '@mantine/core';
import { useThrottledValue } from '@mantine/hooks';
import AppIcon from '../../../apple/client/Assets.xcassets/AppIcon.appiconset/icon_128x128.png';
import { LinuxDegradation, OsStatusWVpnStatus } from '../common/appContext';
import DebugBundle from '../components/DebugBundle';
import ObscuraWordmark from '../components/ObscuraWordmark';
import LinuxServiceDegraded from './LinuxServiceDegraded';

interface SplashScreenProps {
  text?: string;
  osStatus: OsStatusWVpnStatus | null;
  degradation?: LinuxDegradation;
}

export default function SplashScreen({ text = '', osStatus, degradation }: SplashScreenProps) {
  // only show the debug bundle while loading after a prolonged period, but immediately when degraded
  const osStatusThrottled = useThrottledValue(osStatus, 5000);
  return (
    <Stack h='100vh' align='center' justify='center' gap='xl'>
      <Image src={AppIcon} w={64} />
      <ObscuraWordmark />
      {degradation !== undefined
        ? <LinuxServiceDegraded degradation={degradation} />
        : (
          <Group>
            {text.length > 0 && <Text>{text}</Text>}
            <Loader size='xl' type='bars' />
          </Group>
        )}
      {osStatus !== null && (degradation !== undefined || osStatusThrottled !== null) &&
        <DebugBundle osStatus={osStatus} />}
    </Stack>
  );
}

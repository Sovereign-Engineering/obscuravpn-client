import { Group, Image, Loader, Stack, Text } from '@mantine/core';
import { useThrottledValue } from '@mantine/hooks';
import AppIcon from '../../../apple/client/Assets.xcassets/AppIcon.appiconset/icon_128x128.png';
import { IS_HANDHELD_DEVICE } from '../bridge/SystemProvider';
import { OsStatus } from '../common/appContext';
import DebuggingArchive from '../components/DebuggingArchive';
import LogsCapture from '../components/LogCapture';
import ObscuraWordmark from '../components/ObscuraWordmark';

interface SplashScreenProps {
  text: string;
  osStatus: OsStatus | null
}

export default function SplashScreen({ text = '', osStatus }: SplashScreenProps) {
  // only show help UI if loading for a prolonged period of time
  const osStatusThrottled = useThrottledValue(osStatus, 5000);
  return (
    <Stack h='100vh' align='center' justify='center' gap='xl'>
      <Image src={AppIcon} w={64} />
      <ObscuraWordmark />
      <Group>
        {text.length > 0 && <Text>{text}</Text>}
        <Loader size='xl' type='bars' />
      </Group>
      {
        osStatusThrottled !== null &&
        (IS_HANDHELD_DEVICE ? <LogsCapture osStatus={osStatusThrottled} /> : <DebuggingArchive osStatus={osStatusThrottled} />)
      }
    </Stack>
  );
}

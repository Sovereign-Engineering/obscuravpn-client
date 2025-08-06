import { ActionIcon, Group, Text, Tooltip, useComputedColorScheme } from '@mantine/core';
import { SiDiscord, SiMatrix, SiX } from 'react-icons/si';
import { DISCORD_SERVER, MATRIX_SERVER, TWITTER } from '../common/links';
import { useTranslation } from 'react-i18next';

export function Socials() {
  const { t } = useTranslation();
  const colorScheme = useComputedColorScheme();

  return <>
    <Text c='dimmed' ta='center'>
      {t('ConnectWithUs')}
    </Text>
    <Group gap='xl' justify='center'>
      <Tooltip label='Discord'>
        <ActionIcon component='a' href={DISCORD_SERVER} color='#5865f2' size='xl' variant='transparent'>
          <SiDiscord size='100%' />
        </ActionIcon>
      </Tooltip>
      <Tooltip label='Matrix'>
        <ActionIcon component='a' href={MATRIX_SERVER}
          color={colorScheme === 'light' ? 'black' : 'white'} size='xl' variant='transparent'>
          <SiMatrix size='100%' />
        </ActionIcon>
      </Tooltip>
      <Tooltip label='X'>
        <ActionIcon component='a' href={TWITTER} size={50} radius='md' variant='transparent' color={colorScheme === 'light' ? 'black' : 'white'}>
          <SiX size={24} />
        </ActionIcon>
      </Tooltip>
    </Group>
  </>
}

import { ActionIcon, Group, Text, Tooltip } from '@mantine/core';
import { useTranslation } from 'react-i18next';
import { SiDiscord, SiMatrix, SiX } from 'react-icons/si';
import commonClasses from '../common/common.module.css';
import { DISCORD_SERVER, MATRIX_SERVER, TWITTER } from '../common/links';

export function Socials() {
  const { t } = useTranslation();

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
        <ActionIcon component='a' href={MATRIX_SERVER} size='xl' variant='transparent'>
          <SiMatrix className={commonClasses.svgThemed} size='100%' />
        </ActionIcon>
      </Tooltip>
      <Tooltip label='X'>
        <ActionIcon component='a' href={TWITTER} size={50} radius='md' variant='transparent'>
          <SiX className={commonClasses.svgThemed} size={24} />
        </ActionIcon>
      </Tooltip>
    </Group>
  </>
}

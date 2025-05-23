import { ActionIcon, Anchor, Group, Image, Stack, Text, Title, Tooltip, useComputedColorScheme } from '@mantine/core';
import { useContext } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { SiDiscord, SiMatrix } from 'react-icons/si';
import { AppContext } from '../common/appContext';
import { percentEncodeQuery } from '../common/utils';
import DebuggingArchive from '../components/DebuggingArchive';
import MascotThinking from '../res/mascots/thinking-mascot.svg';

const EMAIL = 'support@obscura.net';
const DISCORD_SERVER = 'https://discord.gg/xsP2Fp7s6r';
const MATRIX_SERVER = 'https://matrix.to/#/!CznDYbvmUUGxsJaWuW:matrix.social.obscuravpn.io?via=matrix.social.obscuravpn.io&via=matrix.org'

export default function Help() {
    const { t } = useTranslation();
    const colorScheme = useComputedColorScheme();
    const { osStatus } = useContext(AppContext);

    // \r is important to ensure email clients do not trim newlines
    const params = {
        subject: t('emailSubject', { platform: 'macOS', version: osStatus.srcVersion }),
        body: t('emailBodyIntro') + ':\n\n\r'
    };
    const queryString = percentEncodeQuery(params);
    const mailto = `mailto:${EMAIL}?${queryString}`;

    return <Stack pl={60} pt={40} align='flex-start'>
        <Image src={MascotThinking} w={100} />
        <Title>{t('havingTrouble')}</Title>
        <Text c='gray' component='span'><Trans i18nKey='supportMsg' values={{ email: EMAIL }} components={[<Anchor href={mailto} />]} /></Text>
        <DebuggingArchive />
        <Title order={3}>{t('socials')}</Title>
        <Group gap='xl'>
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
        </Group>
    </Stack>
}

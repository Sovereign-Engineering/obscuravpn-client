import { Anchor, Image, Stack, Text, Title } from '@mantine/core';
import { useContext, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { AppContext } from '../common/appContext';
import { EMAIL } from '../common/links';
import useMailto from '../common/useMailto';
import DebugBundle from '../components/DebugBundle';
import { Socials } from '../components/Socials';
import MascotThinking from '../res/mascots/thinking-mascot.svg';

export default function Help() {
    const { t } = useTranslation();
    const { osStatus } = useContext(AppContext);
    // feedback entered in the debug-bundle dialog is lifted here so the support link includes it
    const [userFeedback, setUserFeedback] = useState('');
    const mailto = useMailto(osStatus, userFeedback);

    return <Stack pl={60} pt={40} align='flex-start'>
        <Image src={MascotThinking} w={100} />
        <Title>{t('havingTrouble')}</Title>
        <Text c='gray' component='span'><Trans i18nKey='supportMsg' values={{ email: EMAIL }} components={[<Anchor href={mailto} />]} /></Text>
        <DebugBundle osStatus={osStatus} feedback={userFeedback} onFeedbackChange={setUserFeedback} />
        <Title order={3}>{t('socials')}</Title>
        <Socials />
    </Stack>
}

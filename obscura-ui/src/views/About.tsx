import { Anchor, Button, Center, Group, Image, Loader, Modal, Stack, Text, Title, useComputedColorScheme, useMantineTheme } from '@mantine/core';
import { lazy, Suspense, useContext, useState } from 'react';
import { useTranslation } from 'react-i18next';
import AppIcon from '../../../apple/client/Assets.xcassets/AppIcon.appiconset/icon_512x512@2x@2x.png';
import { LEGAL_WEBPAGE, OBSCURA_WEBPAGE } from '../common/accountUtils';
import { AppContext } from '../common/appContext';
import Wordmark from '../res/obscura-wordmark.svg?react';

const Licenses = lazy(() => import('../components/Licenses'));

export default function About() {
  const { t } = useTranslation();
  const { osStatus } = useContext(AppContext);
  const colorScheme = useComputedColorScheme();
  const [showLicenses, setShowLicenses] = useState(false);
  const theme = useMantineTheme();

  return (
    <Stack gap='md' align='center' m={40}>
      <Image src={AppIcon} w={120} />
      <Wordmark fill={colorScheme === 'light' ? 'black' : theme.colors.gray[4]} width={150} height='auto' />
      <Text>{osStatus.srcVersion}</Text>
      <Group>
        <Button component='a' href={OBSCURA_WEBPAGE} variant='outline'>{t('Website')}</Button>
      </Group>
      <Text>{t('copyright')}</Text>
      <Modal opened={showLicenses} onClose={() => setShowLicenses(false)} size='lg' title={<Title order={3}>{t('openSourceLicenses')}</Title>}>
        <Suspense fallback={<Center><Loader type='bars' size='md' /></Center>}>
          <Stack>
            <Licenses />
          </Stack>
        </Suspense>
      </Modal>
      <Group>
        <Anchor onClick={() => setShowLicenses(o => !o)}>
          {t('openSourceLicenses')}
        </Anchor>
        <Anchor href={LEGAL_WEBPAGE}>{t('tosAndPrivacyPolicy')}</Anchor>
      </Group>
    </Stack>
  );
}

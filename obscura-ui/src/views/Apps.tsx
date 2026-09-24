import { useTranslation } from 'react-i18next';
import DecoAccent from '../res/deco/deco-bit-accent.svg?react';
import Hat from '../res/mascots/hat.svg?react';
import AppStore from '../res/app-store.svg?react';
import PlayStore from '../res/play-store.svg?react';
import appLight from '../res/app-light.png';
import appDark from '../res/app-dark.png';
import classes from './AppsView.module.css';
import { Card, Flex, Text } from '@mantine/core';
import QrCode from '../components/QRCode';
import { DOWNLOAD_ANDROID_PLAY, DOWNLOAD_IOS } from '../common/links';
import { IS_WEBKIT } from '../bridge/SystemProvider';

export default function Apps() {
  const { t } = useTranslation();

  return (
    <Flex w='100%' mih='100vh' justify='center'>
      <Flex className={classes.container} maw='860px' justify='center' direction='row' gap='48px' w='100%' pos='relative'>
        <DecoAccent className={classes.deco}></DecoAccent>
        <div className={classes.grid}>
          <Flex className={classes.info} gap='16px' direction='column'>
            <Text size='26px' fw='590'>{t('apps-onTheGo')}</Text>
            <Text size='13px' fw='400' c='dimmed' style={{ lineHeight: '16px' }}>{t('apps-scanQRCode')}</Text>
            <Flex className={classes.cards} gap='12px' direction='row'>
              <Card shadow='sm' padding='12px' pt='12px' pb='18px' radius='md' withBorder w='100%' mb='xs'>
                <Flex w='100%' align='center' justify='center' direction='column' gap='24px'>
                  <QrCode value={DOWNLOAD_IOS} type='text' size={114}></QrCode>
                  <a href={DOWNLOAD_IOS}>
                    <AppStore></AppStore>
                  </a>
                </Flex>
              </Card>
              <Card shadow='sm' padding='12px' pt='12px' pb='18px' radius='md' withBorder w='100%' mb='xs'>
                <Flex w='100%' align='center' justify='center' direction='column' gap='24px'>
                  <QrCode value={DOWNLOAD_ANDROID_PLAY} type='text' size={114}></QrCode>
                  <a href={DOWNLOAD_ANDROID_PLAY}>
                    <PlayStore></PlayStore>
                  </a>
                </Flex>
              </Card>
            </Flex>
          </Flex>
          <Flex pos='relative' className={classes.stageWrap}>
            <Flex pos='relative' className={classes.images}>
              <Hat className={classes.hatMascot}></Hat>
              <img src={appDark} className={`${classes.appImage} ${classes.appImageDark}`}></img>
              <img src={appLight} className={`${classes.appImage} ${classes.appImageLight}`} style={{ boxShadow: '171px 237px 82px 0 rgba(0, 0, 0, 0.00), 110px 152px 75px 0 rgba(0, 0, 0, 0.01), 62px 85px 63px 0 rgba(0, 0, 0, 0.03), 27px 38px 47px 0 rgba(0, 0, 0, 0.05), 7px 9px 26px 0 rgba(0, 0, 0, 0.06);' }}></img>
              <div className={`${classes.underShadow} ${IS_WEBKIT ? classes.underShadowApple : ''}`}></div>
            </Flex>
          </Flex>
        </div>
      </Flex>
    </Flex >
  )
}

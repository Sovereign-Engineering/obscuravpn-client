import { Accordion, ActionIcon, Alert, Button, Card, Checkbox, Divider, Group, Radio, Stack, Switch, Text, Title, useMantineColorScheme } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import React, { ReactNode, useContext } from 'react';
import { useTranslation } from 'react-i18next';
import { BsCircleHalf } from 'react-icons/bs';
import { IoInformationCircleOutline, IoMoon, IoSunnySharp } from 'react-icons/io5';
import { MdBlock, MdWarning } from 'react-icons/md';
import * as commands from '../bridge/commands';
import { AppContext, DNSContentBlock, featureFlagEnabled, FeatureFlagKey, KnownFeatureFlagKey } from '../common/appContext';
import commonClasses from '../common/common.module.css';
import { NotificationId } from '../common/notifIds';
import { normalizeError } from '../common/utils';
import { fmtErrorI18n, TranslationKey } from '../translations/i18n';
import classes from './Settings.module.css';

export default function Settings() {
  return (
    <Stack mb='xl' gap='lg' align='flex-start' className={classes.container}>
      <GeneralSettings />
      <DnsSettings />
      <ExperimentalSettings />
      <NetworkSettings />
      <AppearanceSettings />
    </Stack>
  );
}

function DnsSettings() {
  const { t } = useTranslation();
  const { appStatus } = useContext(AppContext);
  const { dnsContentBlock, useSystemDns } = appStatus;

  const handleModeChange = (val: string) => {
    commands.setUseSystemDns(val === 'system');
  };

  const onBlockChange = (key: keyof DNSContentBlock, e: React.ChangeEvent<HTMLInputElement>) => {
    const checked = e.currentTarget.checked;
    const newBlock = { ...dnsContentBlock, [key]: checked };
    commands.setDnsContentBlock(newBlock);
  };

  return (
    <Card padding='md' radius='md' w='100%' shadow='xs'>
      <Stack gap='xs'>
        <Group gap='xs'>
          <MdBlock size='1.5em' style={{ color: 'var(--mantine-color-dimmed)' }} />
          <Title order={4}>{t('dnsSetting')}</Title>
        </Group>

        <Radio.Group value={useSystemDns ? 'system' : 'obscura'} onChange={handleModeChange}>
          <Stack gap='sm'>
            <Radio value="obscura" label={t('dnsModeObscura')} />

            <Stack gap='xs' ml='xl'>
              <Checkbox disabled={useSystemDns} checked={dnsContentBlock.ad} onChange={(e) => onBlockChange('ad', e)} label={t('dnsBlockAds')} />
              <Checkbox disabled={useSystemDns} checked={dnsContentBlock.tracker} onChange={(e) => onBlockChange('tracker', e)} label={t('dnsBlockTrackers')} />
              <Checkbox disabled={useSystemDns} checked={dnsContentBlock.malware} onChange={(e) => onBlockChange('malware', e)} label={t('dnsBlockMalware')} />
              <Checkbox disabled={useSystemDns} checked={dnsContentBlock.gambling} onChange={(e) => onBlockChange('gambling', e)} label={t('dnsBlockGambling')} />
              <Checkbox disabled={useSystemDns} checked={dnsContentBlock.adult} onChange={(e) => onBlockChange('adult', e)} label={t('dnsBlockAdult')} />
              <Checkbox disabled={useSystemDns} checked={dnsContentBlock.socialMedia} onChange={(e) => onBlockChange('socialMedia', e)} label={t('dnsBlockSocialMedia')} />
            </Stack>

            <Radio value="system" label={t('dnsModeSystem')} description={t('dnsModeSystemDescription')} />
          </Stack>
        </Radio.Group>
      </Stack>
    </Card>
  );
}

function GeneralSettings() {
  const { t } = useTranslation();
  const { appStatus, osStatus } = useContext(AppContext);
  const loginItemStatus = osStatus.loginItemStatus;
  const loginItemRegistered = loginItemStatus?.registered;
  const loginItemError = loginItemStatus?.error;

  const registerAtLogin = async () => {
    let success = true;
    try {
      await commands.registerAsLoginItem();
    } catch {
      success = false;
    }
    notifications.hide(NotificationId.OPEN_AT_LOGIN);
    notifications.show({
      id: NotificationId.OPEN_AT_LOGIN,
      title: success ? t('Success') : t('Failed'),
      message: success ? t('openAtLoginEnabled') : t('openAtLoginFailedToEnable'),
      loading: false,
      color: success ? 'green' : 'red'
    });
  }

  const unregisterAtLogin = async () => {
    let success = true;
    try {
      await commands.unregisterAsLoginItem();
    } catch {
      success = false;
    }
    notifications.hide(NotificationId.OPEN_AT_LOGIN);
    notifications.show({
      id: NotificationId.OPEN_AT_LOGIN,
      title: success ? t('Success') : t('Failed'),
      message: success ? t('openAtLoginDisabled') : t('openAtLoginFailedToDisable'),
      loading: false,
      color: success ? 'green' : 'red'
    });
  }

  return (
    <Card padding='md' radius='md' w='100%' shadow='xs'>
      <Stack gap='xs'>
        <Title order={4}>{t('General')}</Title>
        {
          loginItemStatus &&
          <Switch error={loginItemError === undefined ? undefined : loginItemError} disabled={loginItemError !== undefined || loginItemRegistered === undefined} checked={loginItemRegistered} onChange={event => event.currentTarget.checked ? registerAtLogin() : unregisterAtLogin()} label={t('openAtLoginRegister')} />
        }
        <Divider w='100%' />
        <Stack gap={2} w='100%'>
          <Switch checked={appStatus.autoConnect} onChange={event => commands.setAutoConnect(event.currentTarget.checked)} label={t('autoConnectStartup')} description={t('autoConnectStartup-behavior')} />
        </Stack>
      </Stack>
    </Card>
  );
}

function NetworkSettings() {
  const { t } = useTranslation();

  const rotateWgKey = async () => {
    try {
      await commands.rotateWgKey();
    } catch (e) {
      const error = normalizeError(e);
      const message = error instanceof commands.CommandError
        ? fmtErrorI18n(t, error) : error.message;
      notifications.show({
        title: t('Error'),
        message: message,
        color: 'red',
      });
    }
  }

  return (
    <Card padding='md' radius='md' w='100%' shadow='xs'>
      <Stack gap='xs' align='flex-start'>
        <Title order={4}>{t('Network')}</Title>
        <Button onClick={rotateWgKey}>{t('rotateWgKey')}</Button>
      </Stack>
    </Card>
  );
}

function ExperimentalSettings() {
  const { t } = useTranslation();
  const { appStatus } = useContext(AppContext);

  return (
    <Accordion variant='separated' w='100%' classNames={{ item: `${commonClasses.elevatedSurface} ${classes.experimentalAccordionControl}` }}>
      <Accordion.Item value='experimental'>
        <Accordion.Control>
          <Title order={4}>{t('Experimental')}</Title>
        </Accordion.Control>
        <Accordion.Panel style={{ borderTop: '1px solid var(--mantine-color-default-border)' }}>
          <Stack gap='lg' align='flex-start' my='xs'>
            {appStatus.featureFlagKeys.map(featureFlagKey => (
              <React.Fragment key={featureFlagKey}>
                <FeatureFlagToggle featureFlagKey={featureFlagKey} />
                <Divider w='100%' />
              </React.Fragment>
            ))}
            <StrictLeakPreventionSwitch />
          </Stack>
        </Accordion.Panel>
      </Accordion.Item>
    </Accordion>
  );
}

function AppearanceSettings() {
  const { t } = useTranslation();
  const { setColorScheme } = useMantineColorScheme();
  const resetMantineColorScheme = () => setColorScheme('auto');

  return (
    <Card padding='md' radius='md' w='100%' shadow='xs' pb='lg'>
      <Stack gap='lg'>
        <Title order={4}>{t('Appearance')}</Title>
        <Group gap='0' maw='25em' justify='space-around'>
          {colorSchemeOptions.map(({ colorScheme, i18nKey, icon }) => (
            <ActionIcon
              key={colorScheme}
              variant='default'
              onClick={async () => {
                resetMantineColorScheme();
                try {
                  await commands.setColorScheme(colorScheme);
                } catch (e) {
                  console.error('Failed to set theme:', e);
                }
              }}
              h={80}
              w={100}
            >
              <Stack align='center' gap='xs'>
                {icon}
                <Text size='sm'>{t(i18nKey)}</Text>
              </Stack>
            </ActionIcon>
          ))}
        </Group>
      </Stack>
    </Card>
  );
}


function StrictLeakPreventionSwitch() {
  const { t } = useTranslation();
  const { vpnConnected, osStatus } = useContext(AppContext);
  const { strictLeakPrevention } = osStatus;
  const { showLoadingUI, error, execute } = commands.useAsyncCommand();

  const onChange = (checked: boolean) =>
    execute(() => commands.setStrictLeakPrevention(checked));

  const disabled = strictLeakPrevention && vpnConnected;

  return (
    <Stack gap='xs' w='100%'>
      <Switch
        error={error}
        checked={strictLeakPrevention}
        onChange={(event) => onChange(event.currentTarget.checked)}
        disabled={disabled || showLoadingUI}
        label={t('strictLeakPreventionLabel')}
        description={t('strictLeakPreventionDescription')}
      />
      {disabled &&
        <Alert icon={<IoInformationCircleOutline />} color='blue' variant='light'>
          {t('strictLeakPreventionTooltip')}
        </Alert>
      }
      <Alert icon={<MdWarning />} color='orange' variant='light'>
        {t('strictLeakPreventionLanWarning')}
      </Alert>
      <Alert icon={<MdWarning />} color='red' variant='light'>
        {t('strictLeakPreventionReliabilityWarning')}
      </Alert>
    </Stack>
  );
}

function FeatureFlagToggle({ featureFlagKey }: { featureFlagKey: FeatureFlagKey }) {
  const { t, i18n } = useTranslation();
  const { appStatus } = useContext(AppContext);
  const { showLoadingUI, error, execute } = commands.useAsyncCommand();

  const onChange = (checked: boolean) =>
    execute(() => commands.setFeatureFlag(featureFlagKey, checked));

  const labelKey = `featureFlag-${featureFlagKey}-Label`;
  const descriptionKey = `featureFlag-${featureFlagKey}-Description`;

  const label = i18n.exists(labelKey) ? t(labelKey as TranslationKey) : featureFlagKey;
  const description = i18n.exists(descriptionKey) ? t(descriptionKey as TranslationKey) : undefined;

  const additionalComponents = FEATURE_FLAG_CUSTOM_UI[featureFlagKey]?.(t);

  return (
    <Stack gap='xs' w='100%'>
      <Switch
        error={error}
        checked={featureFlagEnabled(appStatus.featureFlags[featureFlagKey])}
        onChange={(event) => onChange(event.currentTarget.checked)}
        disabled={showLoadingUI}
        label={label}
        description={description}
      />
      {additionalComponents}
    </Stack>
  );
}

const FEATURE_FLAG_CUSTOM_UI: Partial<Record<FeatureFlagKey, (t: ReturnType<typeof useTranslation>['t']) => ReactNode>> = {
  [KnownFeatureFlagKey.QuicFramePadding]: (t) => (
    <Alert icon={<MdWarning />} color='orange' variant='light'>
      {t('featureFlag-quicFramePadding-BandwidthWarning')}
    </Alert>
  ),
  [KnownFeatureFlagKey.TcpTlsTunnel]: (t) => (
    <Alert icon={<MdWarning />} color='orange' variant='light'>
      {t('featureFlag-tcpTlsTunnel-BandwidthWarning')}
    </Alert>
  ),
};

const colorSchemeOptions = [
  { colorScheme: 'light', i18nKey: 'Light', icon: <IoSunnySharp size='1.5em' /> },
  { colorScheme: 'dark', i18nKey: 'Dark', icon: <IoMoon size='1.25em' /> },
  { colorScheme: 'auto', i18nKey: 'System', icon: <BsCircleHalf style={{ transform: 'rotate(180deg)' }} size='1.25em' /> }
] as const;

import { Accordion, ActionIcon, Anchor, Button, Card, Divider, Flex, Group, Loader, Space, Stack, Text, ThemeIcon, Title, useMantineTheme } from '@mantine/core';
import { useInterval } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import { getCountryCode } from 'countries-list';
import { MouseEvent, useContext, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { BsPin, BsPinFill, BsShieldFillCheck, BsShieldFillExclamation } from 'react-icons/bs';
import * as commands from '../bridge/commands';
import { Exit, getExitCountry, getContinent } from '../common/api';
import { AppContext, ConnectionInProgress, isConnecting } from '../common/appContext';
import commonClasses from '../common/common.module.css';
import { exitLocation, exitsSortComparator, getExitCountryFlag } from '../common/exitUtils';
import { KeyedSet } from '../common/KeyedSet';
import { NotificationId } from '../common/notifIds';
import { useAsync } from '../common/useAsync';
import { fmtTime, normalizeError } from '../common/utils';
import BoltBadgeAuto from '../components/BoltBadgeAuto';
import ExternalLinkIcon from '../components/ExternalLinkIcon';
import ObscuraChip from '../components/ObscuraChip';
import CheckMarkCircleFill from '../res/checkmark.circle.fill.svg?react';
import Eye from '../res/eye.fill.svg?react';
import EyeSlash from '../res/eye.slash.fill.svg?react';
import OrangeCheckedShield from '../res/orange-checked-shield.svg?react';
import { fmtErrorI18n } from '../translations/i18n';
import classes from './Location.module.css';
import { useExitList } from '../common/useExitList';

export default function LocationView() {
    const { t } = useTranslation();
    const { vpnConnected, vpnConnect, connectionInProgress, appStatus } = useContext(AppContext);

    const { exitList } = useExitList({
      periodS: 60,
    });

    const onExitSelect = async (exit: Exit) => {
        // connected already to the desired exit
        const connectedExit = appStatus.vpnStatus.connected?.exit;
        if (
          exit.country_code === connectedExit?.country_code
          && exit.city_name === connectedExit.city_name
        ) {
            return;
        }
        if (vpnConnected || connectionInProgress !== ConnectionInProgress.UNSET) {
            notifications.show({
                title: t('connectingToCity', { city: exit.city_name }),
                message: '',
                autoClose: 15_000,
                color: 'yellow.4',
                id: NotificationId.VPN_DISCONNECT_CONNECT
            });
        }
        await vpnConnect({
            city: {
                city_code: exit.city_code,
                country_code: exit.country_code,
            },
        });
    };

    const toggleExitPin = (exit: Exit) => {
      if (!exitList) {
        console.error("Toggling pin with no exit list")
        return;
      }

      let removed = appStatus.pinnedLocations.filter(loc =>
        !(loc.city_code == exit.city_code
        && loc.country_code == loc.country_code)
      );
      if (removed.length !== appStatus.pinnedLocations.length) {
        commands.setPinnedExits(removed);
      } else {
        commands.setPinnedExits([
          ...appStatus.pinnedLocations,
          {
            country_code: exit.country_code,
            city_code: exit.city_code,
            pinned_at: Math.floor(Date.now()/1000),
          },
        ]);
      }
    };

    const locations = exitList ?? [];
    const pinnedLocationSet = new KeyedSet(
      (loc: {country_code: string, city_code: string}) => JSON.stringify([loc.country_code, loc.city_code]),
      appStatus.pinnedLocations,
    );
    const pinnedExits = locations.filter(exit => pinnedLocationSet.has(exitLocation(exit)));

    let lastChosenJsx = null;
    if (appStatus.lastChosenExit && "city" in appStatus.lastChosenExit) {
        let lastCity = appStatus.lastChosenExit.city;
        const exit = locations.find(l => l.city_code == lastCity.city_code && l.country_code == lastCity.country_code);
        if (exit !== undefined) {
            const isConnected = appStatus.vpnStatus.connected?.exit.city_code == lastCity.city_code
                && appStatus.vpnStatus.connected.exit.country_code == lastCity.country_code;
            const isPinned = pinnedLocationSet.has(lastCity);
            lastChosenJsx = <>
                <Text ta='left' w='91%' size='sm' c='green.7' ml='md' fw={600}>{t('lastChosen')}</Text>
                <LocationCard exit={exit} togglePin={toggleExitPin}
                    onSelect={() => onExitSelect(exit)} connected={isConnected} pinned={isPinned} />
            </>;
        }
    }

    const pinnedExitsRender = [];
    const insertedCities = new Set(); // [COUNTRY_CODE, CITY]
    if (pinnedExits.length > 0) {
        pinnedExitsRender.push(<Text key='pinned-heading' ta='left' w='91%' size='sm' c='gray' ml='md' fw={700}>{t('Pinned')}</Text>);
        for (const exit of pinnedExits) {
            const key = JSON.stringify([exit.country_code, exit.city_name]);
            if (!insertedCities.has(key)) {
              insertedCities.add(key);
              const isConnected = exit.id === appStatus.vpnStatus.connected?.exit.id;
              const isPinned = pinnedLocationSet.has(exitLocation(exit));
              pinnedExitsRender.push(<LocationCard key={key} exit={exit} togglePin={toggleExitPin}
                  onSelect={() => onExitSelect(exit)} connected={isConnected} pinned={isPinned} />);
            }
        }
    }

    const exitListRender = [];
    const insertedContinents = new Set();
    insertedCities.clear();
    const connectedExit = appStatus.vpnStatus.connected?.exit;

    locations.sort(exitsSortComparator);
    for (const exit of locations) {
        const continent = getContinent(getExitCountry(exit));
        if (!insertedContinents.has(continent)) {
            exitListRender.push(<Text key={`continent-${continent}`} ta='left' w='91%' size='sm' c='gray' ml='sm' fw={600}>{t(`Continent${continent}`)}</Text>);
            insertedContinents.add(continent);
        }
        const key = JSON.stringify([exit.country_code, exit.city_name]);
        if (!insertedCities.has(key)) {
          insertedCities.add(key);
          const isConnected = exit.country_code === connectedExit?.country_code && exit.city_name == connectedExit?.city_name;
          const isPinned = pinnedLocationSet.has(exitLocation(exit));
          exitListRender.push(<LocationCard key={key} exit={exit} togglePin={toggleExitPin}
              onSelect={() => onExitSelect(exit)} connected={isConnected} pinned={isPinned} />);
        }
    }

    return (
        <Stack align='center' gap={10} p={20} mt='sm'>
            <VpnStatusCard />
            {locations.length === 0 ? <NoExitServers /> :
                <>
                    {lastChosenJsx}
                    {pinnedExitsRender}
                    {exitListRender}
                </>}
        </Stack>
    );
}

interface LocationCarProps {
  exit: Exit,
  connected: boolean,
  onSelect: () => void,
  togglePin: (exit: Exit) => void,
  pinned: boolean
}

function LocationCard({ exit, connected, onSelect, togglePin, pinned }: LocationCarProps) {
    const { t } = useTranslation();
    const { connectionInProgress, osStatus, vpnConnected } = useContext(AppContext);
    const { internetAvailable } = osStatus;
    const isOffline = !internetAvailable && !vpnConnected && !isConnecting(connectionInProgress);

    const onPinClick = (e: MouseEvent) => {
        e.stopPropagation();
        togglePin(exit);
    };

    const disableClick = connectionInProgress !== ConnectionInProgress.UNSET || isOffline;
    const cardClasses = [];
    if (connected) cardClasses.push(classes.locationCardConnected);
    if (disableClick) cardClasses.push(classes.locationCardDisabled);
    else if (!connected) cardClasses.push(classes.locationCardNotConnected);
    const cardTitle = (!connected && !disableClick) ? t('Click to connect') : undefined;

    return (
        <Card shadow='xs' title={cardTitle} className={cardClasses.join(' ')} withBorder padding='xs' radius='md' w='90%' onClick={(connected || disableClick) ? undefined : onSelect}>
            <Group justify='space-between'>
                <Group>
                    <Text size='2rem'>{getExitCountryFlag(exit)}</Text>
                    <Flex direction='column'>
                        <Text size='md'>{exit.city_name}</Text>
                        <Text c='dimmed' size='sm'>{getExitCountry(exit).name}</Text>
                    </Flex>
                </Group>
                <Group>
                    {connected && <ObscuraChip>{t('Connected')}</ObscuraChip>}
                    <ActionIcon className={classes.favoriteBtn} variant={pinned ? 'gradient' : 'outline'} title={pinned ? 'unpin exit' : 'pin exit'} color={pinned ? 'orange' : 'gray'} onClick={onPinClick}>
                        {pinned ? <BsPinFill size='1rem' /> : <BsPin size='1rem' />}
                    </ActionIcon>
                </Group>
            </Group>
        </Card>
    );
}

function NoExitServers() {
    const { t } = useTranslation();
    const [isLoading, setIsLoading] = useState(false);

    if (isLoading) {
        return <Loader mt={10} type='dots' />;
    }

    async function refetch() {
        try {
            setIsLoading(true);
            console.log("Fetching exits");
            await commands.refreshExitList(0);
        } catch (error) {
          let message = error instanceof commands.CommandError
            ? fmtErrorI18n(t, error)
            : t('exitServerFetchResolution');

          notifications.show({
            id: 'failedToFetchExitServers',
            title: t('failedToFetchExitServers'),
            message,
            color: 'red',
          });
        } finally {
            setIsLoading(false);
        }
    }

    return (
        <Card shadow='sm' padding='lg' radius='md' withBorder w='90%'>
            <Group justify='space-between' >
                <Group align='center' gap={5}>
                    <Text size='xl' fw={700} c='red.7'>
                        {t('noExitServers')}
                    </Text>
                </Group>
                <Button onClick={refetch} color='green.7' radius='md' variant='filled'>
                    {t('refetchExitList')}
                </Button>
            </Group>
        </Card>
    );
}

function VpnStatusCard() {
    const theme = useMantineTheme();
    const { t } = useTranslation();
    const { appStatus, vpnConnected, connectionInProgress, vpnDisconnect, vpnConnect, osStatus } = useContext(AppContext);
    const { internetAvailable } = osStatus;
    const isOffline = !internetAvailable && !vpnConnected && !isConnecting(connectionInProgress);

    const getStatusTitle = () => {
        if (isOffline) return t('Offline');
        if (connectionInProgress !== ConnectionInProgress.UNSET) return t(connectionInProgress) + '...';
        const selectedLocation = appStatus.vpnStatus.connected?.exit.city_name;
        // vpnConnected <-> vpnStatus.connected.exit defined
        if (selectedLocation !== undefined) return t('connectedToLocation', { location: selectedLocation });
        return t('Disconnected');
    };

    const getStatusSubtitle = () => {
        if (isOffline) return t('connectToInternet');
        if (connectionInProgress === ConnectionInProgress.ChangingLocations) {
          return t('trafficSuspended');
        }
        return (vpnConnected && connectionInProgress === ConnectionInProgress.UNSET) ? t('trafficProtected') : t('trafficVulnerable');
    };

    const allowCancel = connectionInProgress === ConnectionInProgress.Connecting || connectionInProgress === ConnectionInProgress.Reconnecting;
    const btnDisabled = !allowCancel && (connectionInProgress === ConnectionInProgress.Disconnecting || connectionInProgress === ConnectionInProgress.ChangingLocations || isOffline);
    const buttonDisconnectProps = ((allowCancel || vpnConnected) && !btnDisabled) ? theme.other.buttonDisconnectProps : {};

    const getButtonContent = () => {
        if (allowCancel) return t('Cancel Connecting');
        if (connectionInProgress !== ConnectionInProgress.UNSET) return t(connectionInProgress) + '...';
        if (vpnConnected) return t('Disconnect');
        return <Group gap={5} ml={0}><BoltBadgeAuto />{t('QuickConnect')}</Group>;
    };

    const btnTitle = () => {
        if (!btnDisabled) return;
        if (isOffline) return t('noInternet');
        return t('busyConnection');
    };

    const statusColor = (vpnConnected && connectionInProgress === ConnectionInProgress.UNSET) ? 'green.7'
        : (connectionInProgress === ConnectionInProgress.ChangingLocations ? 'gray' : 'red.7');

    return (
        <Card shadow='sm' padding='lg' radius='md' withBorder w='90%' mb='xs'>
            <Group justify='space-between'>
                <Stack gap={0}>
                    <Group align='center' gap={5}>
                        <ThemeIcon color={statusColor} variant='transparent'>
                            {connectionInProgress === ConnectionInProgress.ChangingLocations
                            ? <Loader size='xs' color='gray.5' /> : (vpnConnected && connectionInProgress === ConnectionInProgress.UNSET)
                            ? <BsShieldFillCheck size={25} />
                            : <BsShieldFillExclamation size={25} />}
                        </ThemeIcon>
                    <Title order={4} fw={600} c={statusColor}>{getStatusTitle()}</Title>
                    </Group>
                    <Text c='gray' size='sm' ml={34}>{getStatusSubtitle()}</Text>
                </Stack>
                <Button
                  className={commonClasses.button}
                  miw={190}
                  onClick={() => {
                    if (vpnConnected || allowCancel) {
                      vpnDisconnect();
                    } else {
                      vpnConnect({ any: {} });
                    }
                  }}
                  disabled={btnDisabled}
                  title={btnTitle()}
                  px={10}
                  radius='md'
                  {...buttonDisconnectProps}
                >
                    {getButtonContent()}
                </Button>
            </Group>
            {appStatus.vpnStatus.connected !== undefined && connectionInProgress !== ConnectionInProgress.Disconnecting &&
              <>
                <Divider my='md' />
                <Stack justify='space-between' w='100%'>
                  <CurrentSession />
                  <ExitInfoCollapse exitPubKey={appStatus.vpnStatus.connected.exitPublicKey} connectedExit={appStatus.vpnStatus.connected.exit} />
                </Stack>
              </>
            }
        </Card>
    );
}

function CurrentSession() {
  const { t } = useTranslation();
  const { value: trafficStats, refresh: pollTrafficStats } = useAsync({ deps: [], load: commands.getTrafficStats });
  useInterval(pollTrafficStats, 1000, { autoInvoke: true });
  if (trafficStats === undefined) return;
  return (
    <Stack gap={5}>
      <Text c='gray' size='sm'>{t('currentSession')}</Text>
      <Group>
        <Text size='sm'>{fmtTime(trafficStats.connectedMs)}</Text>
      </Group>
    </Stack>
  );
}

function ExitInfoCollapse({ exitPubKey, connectedExit }: { exitPubKey: string, connectedExit: Exit}) {
  const theme = useMantineTheme();
  const { t } = useTranslation();
  const [showExitInfo, setShowExitInfo] = useState(false);

  const exitProviderId = connectedExit.provider_id;
  const exitProviderURL = connectedExit.provider_url;
  const provider = connectedExit.provider_name;
  const providerUrl = connectedExit.provider_homepage_url;

  return (
    <Accordion classNames={{item: classes.item}} variant='contained' value={showExitInfo ? '0' : null} onChange={() => setShowExitInfo(!showExitInfo)} chevron={null} disableChevronRotation>
      <Accordion.Item key='0' value='0'>
        <Accordion.Control icon={<OrangeCheckedShield height='1em' width='1em' />}>
          <Group justify='space-between'>
            <Text size='sm' c='gray'><Trans i18nKey='securedBy' values={{ provider }} components={[<Anchor onClick={e => e.stopPropagation()} c='orange' href={providerUrl} />]} /></Text>
            <Group style={{alignItems: 'center'}} gap={5}>
              {showExitInfo ?
                <EyeSlash fill={theme.other.dimmed} width='1em' height='1em' /> :
                <Eye fill={theme.other.dimmed} width='1em' height='1em' />}
              <Text size='xs' c='dimmed'>{t(showExitInfo ? 'serverInfoHide' : 'serverInfoReveal')}</Text>
            </Group>
          </Group>
        </Accordion.Control>
        <Accordion.Panel>
          <Stack gap={2}>
            <Stack gap={5}>
              <Text c='gray' size='sm'>{t('Node')}</Text>
              <Text size='sm'>
                {exitProviderId}
                <Text span c='dimmed' size='sm'> ({connectedExit.city_name}, {getExitCountry(connectedExit).name})</Text>
              </Text>
            </Stack>
            <Space h='xs' />
            <Stack gap={5}>
              <Text c='gray' size='sm'>{t('publicKey')}</Text>
              <Group gap={3}>
                <Text ff='monospace' size='sm'>{exitPubKey}</Text>
              </Group>
              <Text size='xs' c='green.7' fw={500}><Trans i18nKey='exitPubKeyTooltip' values={{ exitProviderId, exitProviderURL }} components={[<CheckMarkCircleFill height={11} width={11} fill={theme.colors.green[7]} style={{ marginBottom: -1 }} />, <Anchor underline='always' href={exitProviderURL} />, <ThemeIcon variant='transparent' size={12}><ExternalLinkIcon style={{ height: '100%' }} /></ThemeIcon>]} /></Text>
            </Stack>
          </Stack>
        </Accordion.Panel>
      </Accordion.Item>
    </Accordion>
  );
}

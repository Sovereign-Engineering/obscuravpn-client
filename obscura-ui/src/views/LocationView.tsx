import { ActionIcon, Button, Card, Divider, Flex, Group, Loader, Stack, Text, TextInput, ThemeIcon, useMantineTheme } from '@mantine/core';
import { useInterval } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import { continents } from 'countries-list';
import { MouseEvent, useContext, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BsPin, BsPinFill, BsShieldFillCheck, BsShieldFillExclamation } from 'react-icons/bs';
import { FaEye, FaEyeSlash } from 'react-icons/fa';
import * as commands from '../bridge/commands';
import { Exit, getExitCountry } from '../common/api';
import { AppContext, ConnectionInProgress, ExitsContext } from '../common/appContext';
import commonClasses from '../common/common.module.css';
import { fmtErrorI18n } from '../common/danger';
import { CityNotFoundError, exitLocation, exitsSortComparator, getExitCountryFlag, getRandomExitFromCity } from '../common/exitUtils';
import { KeyedSet } from '../common/KeyedSet';
import { NotificationId } from '../common/notifIds';
import { useAsync } from '../common/useAsync';
import { fmtTime, normalizeError } from '../common/utils';
import BoltBadgeAuto from '../components/BoltBadgeAuto';
import ObscuraChip from '../components/ObscuraChip';
import GoldCheckedBadge from '../res/gold-checked-badge.svg?react';
import classes from './Location.module.css';

export default function LocationView() {
    const { t } = useTranslation();
    const { vpnConnected, vpnConnect, connectionInProgress, vpnDisconnectConnect, appStatus } = useContext(AppContext);
    const { exitList } = useContext(ExitsContext);

    const onExitSelect = async (exit: Exit) => {
        // connected already to the desired exit
        const connectedExit = appStatus.vpnStatus.connected?.exit;
        if (exit.country_code === connectedExit?.country_code &&
          exit.city_name === connectedExit.city_name) return;
        if (vpnConnected || connectionInProgress !== ConnectionInProgress.UNSET) {
            notifications.show({
                title: t('connectingToCity', { city: exit.city_name }),
                message: '',
                autoClose: 15_000,
                color: 'yellow',
                id: NotificationId.VPN_DISCONNECT_CONNECT
            });
            await vpnDisconnectConnect(exit.id);
        } else if (exitList !== null) {
            try {
              exit = getRandomExitFromCity(exitList, exit.country_code, exit.city_code);
            } catch (error) {
              const e = normalizeError(error);
              if (e instanceof CityNotFoundError) {
                  const countryName = getExitCountry(exit).name;
                      notifications.show({
                          title: t('Error'),
                          message: t('noExitsFoundMatching', { country: countryName, city: exit.city_name }),
                          color: 'red',
                      });
              } else {
                  notifications.show({
                      title: t('Error'),
                      message: e.message,
                      color: 'red',
                  });
              }
            }
            await vpnConnect(exit.id);
        }
    };

    const toggleExitPin = (exit: Exit) => {
      if (exitList === null) {
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

    const locations = exitList === null ? [] : exitList;
    const pinnedLocationSet = new KeyedSet(
      loc => JSON.stringify([loc.country_code, loc.city_code]),
      appStatus.pinnedLocations,
    );
    const pinnedExits = locations.filter(exit => pinnedLocationSet.has(exitLocation(exit)));

    let lastChosenJsx = null;
    if (appStatus.lastChosenExit !== null) {
        const exit = locations.find((value) => value.id === appStatus.lastChosenExit);
        if (exit !== undefined) {
            const isConnected = exit.id === appStatus.vpnStatus.connected?.exit.id;
            const isPinned = pinnedLocationSet.has(exitLocation(exit));
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

    locations.sort(exitsSortComparator(null, null, []));
    for (const exit of locations) {
        const continent = getExitCountry(exit).continent;
        if (!insertedContinents.has(continent)) {
            exitListRender.push(<Text key={`continent-${continent}`} ta='left' w='91%' size='sm' c='gray' ml='sm' fw={600}>{continents[continent]}</Text>);
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
    const { connectionInProgress, osStatus } = useContext(AppContext);
    const { internetAvailable } = osStatus;

    const onPinClick = (e: MouseEvent) => {
        e.stopPropagation();
        togglePin(exit);
    };

    const disableClick = connectionInProgress !== ConnectionInProgress.UNSET || !internetAvailable;
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
    const { fetchExitList } = useContext(ExitsContext);
    const [isLoading, setIsLoading] = useState(false);

    if (isLoading) {
        return <Loader mt={10} type='dots' />;
    }

    async function refetch() {
        try {
            setIsLoading(true);
            console.log("Fetching exits");
            await fetchExitList();
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
                <Button onClick={refetch} color='teal' radius='md' variant='filled'>
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
    const exitPubKey = appStatus.vpnStatus.connected?.exit_public_key;
    const [showExitPubKey, setShowExitPubKey] = useState(false);
    const { value: trafficStats, refresh: pollTrafficStats } = useAsync({deps: [], load: commands.getTrafficStats });
    useInterval(pollTrafficStats, 1000, { autoInvoke: true });

    const getStatusTitle = () => {
        if (!internetAvailable) return t('Offline');
        if (connectionInProgress !== ConnectionInProgress.UNSET && connectionInProgress !== ConnectionInProgress.ChangingLocations) return t(connectionInProgress) + '...';
        const selectedLocation = appStatus.vpnStatus.connected?.exit.city_name;
        // vpnConnected <-> vpnStatus.connected.exit defined
        if (selectedLocation !== undefined) return t('connectedToLocation', { location: selectedLocation });
        return t('Disconnected');
    };

    const getStatusSubtitle = () => {
        if (!internetAvailable) return t('connectToInternet');
        return vpnConnected ? t('trafficProtected') : t('trafficVulnerable');
    };

    const allowCancel = connectionInProgress === ConnectionInProgress.Connecting || connectionInProgress === ConnectionInProgress.Reconnecting;
    const btnDisabled = !allowCancel && (connectionInProgress === ConnectionInProgress.Disconnecting || connectionInProgress === ConnectionInProgress.ChangingLocations || (!vpnConnected && !internetAvailable));
    const buttonDisconnectProps = ((allowCancel || vpnConnected) && !btnDisabled) ? theme.other.buttonDisconnectProps : {};

    const getButtonContent = () => {
        if (allowCancel) return t('Cancel Connecting');
        if (connectionInProgress !== ConnectionInProgress.UNSET) return t(connectionInProgress) + '...';
        if (vpnConnected) return t('Disconnect');
        return <Group gap={5} ml={0}><BoltBadgeAuto />{t('QuickConnect')}</Group>;
    };

    const btnTitle = () => {
        if (!btnDisabled) return;
        if (!internetAvailable) return t('noInternet');
        return t('busyConnection');
    };

    return (
        <Card shadow='sm' padding='lg' radius='md' withBorder w='90%' mb='xs'>
            <Group justify='space-between'>
                <Group align='center' gap={5}>
                    <ThemeIcon color={vpnConnected ? 'teal' : 'red.7'} variant='transparent'>
                        {vpnConnected ? <BsShieldFillCheck size={25} /> : <BsShieldFillExclamation size={25} />}
                    </ThemeIcon>
                    <Text size='xl' fw={700} c={vpnConnected ? 'teal' : 'red.7'}>{getStatusTitle()}</Text>
                </Group>
                <Button className={commonClasses.button} miw={190} onClick={(_: MouseEvent) => (vpnConnected || allowCancel) ? vpnDisconnect() : vpnConnect()} disabled={btnDisabled} title={btnTitle()} px={10} radius='md' {...buttonDisconnectProps}>
                    {getButtonContent()}
                </Button>
            </Group>
            <Text c='dimmed' size='sm' ml={34}>{getStatusSubtitle()}</Text>
            {exitPubKey !== undefined &&
              <>
                <Divider ml={34} my='md' />
                <Group ml={34} justify='space-between' w='100%'>
                  {trafficStats?.connectedMs !== undefined &&
                    <Stack gap={5}>
                      <Text c='dimmed' size='sm'>{t('currentSession')}</Text>
                      <Group>
                        <Text size='sm'>{fmtTime(trafficStats?.connectedMs)}</Text>
                      </Group>
                    </Stack>
                  }
                  <Stack gap={5}>
                    <Group h='1em' gap={3}>
                      <Text c='dimmed' size='sm'>{t('exitPubKey')}</Text>
                      <ActionIcon variant='subtle' onClick={() => setShowExitPubKey(!showExitPubKey)}>
                        {showExitPubKey ? <FaEyeSlash size='1em'/> : <FaEye size='1em'/>}
                      </ActionIcon>
                    </Group>
                    <Group w={`${exitPubKey.length + 2}ch`} gap={3}>
                      {showExitPubKey ? <GoldCheckedBadge /> : ''}<TextInput readOnly w={`${exitPubKey.length}ch`} variant='unstyled' size='sm' value={exitPubKey} type={showExitPubKey ? 'text' : 'password'} />
                    </Group>
                  </Stack>
                </Group>
              </>
            }
        </Card>
    );
}

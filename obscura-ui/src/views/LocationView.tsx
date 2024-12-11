import { ActionIcon, Button, Card, Flex, Group, Loader, Space, Stack, Text, ThemeIcon, useMantineTheme } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import { continents } from 'countries-list';
import { MouseEvent, useContext, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BsPin, BsPinFill, BsShieldFillCheck, BsShieldFillExclamation } from 'react-icons/bs';

import * as commands from '../bridge/commands';
import { Exit, getExitCountry } from '../common/api';
import { AppContext, ConnectionInProgress, ExitsContext } from '../common/appContext';
import commonClasses from '../common/common.module.css';
import { getErrorI18n } from '../common/danger';
import { exitsSortComparator, getExitCountryFlag } from '../common/exitUtils';
import { NotificationId } from '../common/notifIds';
import BoltBadgeAuto from '../components/BoltBadgeAuto';
import ObscuraChip from '../components/ObscuraChip';
import classes from './Location.module.css';

export default function LocationView() {
    const { t } = useTranslation();
    const { vpnConnected, vpnConnect, connectionInProgress, vpnDisconnectConnect, appStatus } = useContext(AppContext);
    const { exitList } = useContext(ExitsContext);

    const connectedExitId = appStatus.vpnStatus?.connected?.exit?.id;

    const onExitSelect = async (n: Exit) => {
        if (n.id === connectedExitId) return;
        if (vpnConnected || connectionInProgress !== ConnectionInProgress.UNSET) {
            notifications.show({
                title: t('connectingToCity', { city: n.city_name }),
                message: '',
                autoClose: 15_000,
                color: 'yellow',
                id: NotificationId.VPN_DISCONNECT_CONNECT
            });
            await vpnDisconnectConnect(n.id);
        } else {
            await vpnConnect(n.id);
        }
    }

    // set from locations.map
    let selectedLocation = null;

    if (exitList !== null) {
        for (const exitNode of exitList) {
            if (exitNode.id === connectedExitId) {
                selectedLocation = exitNode.city_name;
                break;
            }
        }
    }

    const toggleExitPin = (exitId: string) => {
        // remove from list if already pinned
        // else, add to list
        const newPinnedExits = [...appStatus.pinnedExits];
        const existingIndex = newPinnedExits.indexOf(exitId);
        if (existingIndex === -1) {
            newPinnedExits.push(exitId);
        } else {
            newPinnedExits.splice(existingIndex, 1);
        }
        commands.setPinnedExits(newPinnedExits);
    }

    const locations = exitList === null ? [] : exitList;
    const pinnedExitsSet = new Set(appStatus.pinnedExits);
    const pinnedExits = locations.filter(exit => pinnedExitsSet.has(exit.id));

    let lastChosenJsx = null;
    if (appStatus.lastChosenExit !== null) {
        const exit = locations.find((value) => value.id === appStatus.lastChosenExit);
        if (exit !== undefined) {
            const isConnected = exit.id === appStatus.vpnStatus?.connected?.exit.id;
            const isPinned = pinnedExitsSet.has(exit.id);
            lastChosenJsx = <>
                <Text ta='left' w='91%' size='sm' c='green.7' ml='md' fw={600}>{t('lastChosen')}</Text>
                <LocationCard exit={exit} togglePin={toggleExitPin}
                    onSelect={() => onExitSelect(exit)} connected={isConnected} pinned={isPinned} />
                <Space />
            </>;
        }
    }

    const pinnedExitsRender = [];
    if (pinnedExits.length > 0) {
        pinnedExitsRender.push(<Text key='pinned-heading' ta='left' w='91%' size='sm' c='gray' ml='md' fw={700}>{t('Pinned')}</Text>);
        for (const exit of pinnedExits) {
            const isConnected = exit.id === appStatus.vpnStatus?.connected?.exit.id;
            const isPinned = pinnedExitsSet.has(exit.id);
            pinnedExitsRender.push(<LocationCard key={exit.id} exit={exit} togglePin={toggleExitPin}
                onSelect={() => onExitSelect(exit)} connected={isConnected} pinned={isPinned} />);
        }
        pinnedExitsRender.push(<Space key='space-pinned' />);
    }

    const exitListRender = [];
    const insertedContinents = new Set();
    locations.sort(exitsSortComparator(null, null, []));
    for (const exit of locations) {
        const continent = getExitCountry(exit).continent;
        if (!insertedContinents.has(continent)) {
            if (insertedContinents.size > 0) {
                exitListRender.push(<Space key={`space-${continent}`} />);
            }
            exitListRender.push(<Text key={`continent-${continent}`} ta='left' w='91%' size='sm' c='gray' ml='sm' fw={600}>{continents[continent]}</Text>);
            insertedContinents.add(continent);
        }
        const isConnected = exit.id === appStatus.vpnStatus?.connected?.exit.id;
        const isPinned = pinnedExitsSet.has(exit.id);
        exitListRender.push(<LocationCard key={exit.id} exit={exit} togglePin={toggleExitPin}
            onSelect={() => onExitSelect(exit)} connected={isConnected} pinned={isPinned} />);
    }

    return (
        <Stack align='center' gap={10} p={20} mt='sm'>
            <VpnStatusCard selectedLocation={selectedLocation} />
            <Space />
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
  togglePin: (exit: string) => void,
  pinned: boolean
}

function LocationCard({ exit, connected, onSelect, togglePin, pinned }: LocationCarProps) {
    const { t } = useTranslation();
    const { connectionInProgress, osStatus } = useContext(AppContext);
    const { internetAvailable } = osStatus;

    const onPinClick = (e: MouseEvent) => {
        e.stopPropagation();
        togglePin(exit.id);
    }

    const disableClick = connectionInProgress !== ConnectionInProgress.UNSET || !internetAvailable;
    const cardClasses = [];
    if (connected) cardClasses.push(classes.locationCardConnected);
    if (disableClick) cardClasses.push(classes.locationCardDisabled);
    else if (!connected) cardClasses.push(classes.locationCardNotConnected);
    const cardTitle = (!connected && !disableClick) ? t('Click to connect') : undefined;

    return (
        <Card title={cardTitle} className={cardClasses.join(' ')} withBorder padding='lg' radius='md' w='90%' onClick={(connected || disableClick) ? undefined : onSelect}>
            <Group justify='space-between'>
                <Group>
                    <Text size='2rem'>{getExitCountryFlag(exit)}</Text>
                    <Flex direction='column'>
                        <Text fw={500} size='lg'>{exit.city_name}</Text>
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
            ? getErrorI18n(t, error)
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

function VpnStatusCard({ selectedLocation }: { selectedLocation: string | null }) {
    const theme = useMantineTheme();
    const { t } = useTranslation();
    const { vpnConnected, connectionInProgress, toggleVpnConnection, osStatus } = useContext(AppContext);
    const { internetAvailable } = osStatus;

    const getStatusTitle = () => {
        if (!internetAvailable) return t('Offline');
        if (connectionInProgress === ConnectionInProgress.Disconnecting) return t(connectionInProgress) + '...';
        if (vpnConnected) return selectedLocation === null ? t('Connected') : t('connectedToLocation', { location: selectedLocation });
        return t('Disconnected');
    };

    const getStatusSubtitle = () => {
        if (!internetAvailable) return t('connectToInternet');
        return vpnConnected ? t('Traffic is protected') : t('Traffic is vulnerable');
    }

    const btnDisabled = connectionInProgress !== ConnectionInProgress.UNSET || !internetAvailable;
    const buttonDisconnectProps = (vpnConnected && !btnDisabled) ? theme.other.buttonDisconnectProps : {};

    const connectionTransition = connectionInProgress !== ConnectionInProgress.UNSET;
    const getButtonContent = () => {
        if (connectionTransition) return t(connectionInProgress) + '...'
        if (vpnConnected) return t('Disconnect');
        return <Group gap={5} ml={0}><BoltBadgeAuto />{t('Quick Connect')}</Group>;
    }

    const btnTitle = () => {
      if (!btnDisabled) return;
      if (!internetAvailable) return t('noInternet');
      return t('busyConnection');
    }

    return (
        <Card shadow='sm' padding='lg' radius='md' withBorder w='90%'>
            <Group justify='space-between' >
                <Group align='center' gap={5}>
                    <ThemeIcon color={vpnConnected ? 'teal' : 'red.7'} variant='transparent'>
                        {vpnConnected ? <BsShieldFillCheck size={25} /> : <BsShieldFillExclamation size={25} />}
                    </ThemeIcon>
                    <Text size='xl' fw={700} c={vpnConnected ? 'teal' : 'red.7'}>{getStatusTitle()}</Text>
                </Group>
                <Button className={commonClasses.button} miw={190} onClick={toggleVpnConnection} disabled={btnDisabled} title={btnTitle()} px={10} radius='md' {...buttonDisconnectProps}>
                    {getButtonContent()}
                </Button>
            </Group>
            <Text c='dimmed' size='sm' ml={34}>{getStatusSubtitle()}</Text>
        </Card>
    );
}

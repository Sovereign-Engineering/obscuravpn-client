// component example
import { Anchor, Button, Combobox, Divider, Group, Image, Paper, Progress, ScrollArea, Space, Stack, Text, ThemeIcon, Title, useComputedColorScheme, useMantineTheme } from '@mantine/core';
import { useInterval, useToggle } from '@mantine/hooks';
import { continents, countries } from 'countries-list';
import { useContext, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BsChevronDown, BsPinFill } from 'react-icons/bs';
import { FaExternalLinkAlt } from 'react-icons/fa';
import { IoIosEyeOff } from 'react-icons/io';
import { MdLanguage, MdLaptopMac, MdOutlineWifiOff } from 'react-icons/md';

import { CHECK_STATUS_WEBPAGE } from '../common/accountUtils';
import { AppContext, ConnectingStrings, ExitsContext, isConnecting } from '../common/appContext';
import { countryCodeToFlagEmoji, exitsSortComparator } from '../common/exitUtils';
import { useCookie } from '../common/utils';
import BoltBadgeAuto from '../components/BoltBadgeAuto';
import ObscuraChip from '../components/ObscuraChip';
import DecoConnected from '../res/deco/deco-connected.svg';
import DecoConnectingDark1 from '../res/deco/deco-connecting-dark-1.svg';
import DecoConnectingDark2 from '../res/deco/deco-connecting-dark-2.svg';
import DecoConnectingDark3 from '../res/deco/deco-connecting-dark-3.svg';
import DecoConnectingLight1 from '../res/deco/deco-connecting-light-1.svg';
import DecoConnectingLight2 from '../res/deco/deco-connecting-light-2.svg';
import DecoConnectingLight3 from '../res/deco/deco-connecting-light-3.svg';
import DecoDisconnectedDark from '../res/deco/deco-disconnected-dark.svg';
import DecoDisconnectedLight from '../res/deco/deco-disconnected-light.svg';
import DecoOfflineDark from '../res/deco/deco-offline-dark.svg';
import DecoOfflineLight from '../res/deco/deco-offline-light.svg';
import MascotConnectedFirstTime from '../res/mascots/connected-first-time-mascot.svg';
import MascotConnected from '../res/mascots/connected-mascot.svg';
import MascotConnecting1 from '../res/mascots/connecting-1-mascot.svg';
import MascotConnecting2 from '../res/mascots/connecting-2-mascot.svg';
import MascotConnecting3 from '../res/mascots/connecting-3-mascot.svg';
import MascotConnecting4 from '../res/mascots/connecting-4-mascot.svg';
import MascotNoInternet from '../res/mascots/no-internet-mascot.svg';
import MascotNotConnected from '../res/mascots/not-connected-mascot.svg';
import ObscuraIconHappy from '../res/obscura-icon-happy.svg';

import commonClasses from '../common/common.module.css';
import classes from './ConnectionView.module.css';

// Los Angeles, CA
const BUTTON_WIDTH = 320;

export default function Connection() {
    const theme = useMantineTheme();
    const colorScheme = useComputedColorScheme();
    const { t } = useTranslation();
    const { vpnConnected, connectionInProgress, osStatus, vpnConnect, vpnDisconnect } = useContext(AppContext);
    const { internetAvailable } = osStatus;
    const connectionTransition = connectionInProgress !== ConnectingStrings.UNSET;
    const [cityConnectingTo, setCityConnectingTo] = useState(null);

    useEffect(() => {
        if (!vpnConnected && !isConnecting(connectionInProgress)) {
            setCityConnectingTo(null);
        }
    }, [vpnConnected, connectionInProgress]);

    const getTitle = () => {
        if (!internetAvailable) return t('Disconnected');
        if (connectionTransition) {
            switch (connectionInProgress) {
                case ConnectingStrings.connecting:
                case ConnectingStrings.changingLocations:
                    return t('connectingTo', { location: cityConnectingTo ?? 'Obscura' });
                case ConnectingStrings.reconnecting:
                    return t('Reconnecting to Obscura');
                case ConnectingStrings.disconnecting:
                    return t('Disconnecting');
            }
        }
        if (vpnConnected) return t('connectedToObscura');
        return t('notConnected');
    }

    const Subtitle = () => {
        if (!internetAvailable) return t('connectToInternet');

        if (connectionTransition) return t('pleaseWaitAMoment');

        if (vpnConnected) return t('enjoyObscura');
        return t('connectToEnjoy');
    }

    const getButtonContent = () => {
        if (connectionTransition) return t(connectionInProgress) + '...'
        return <Group gap={5}><BoltBadgeAuto />{t('Quick Connect')}</Group>;
    }

    // qc: Quick Connect
    const qcBtnAction = e => vpnConnected ? vpnDisconnect() : vpnConnect();
    const qcBtnDisabled = !internetAvailable || connectionTransition;
    const primaryBtnDisconnectProps = (vpnConnected && connectionInProgress !== ConnectingStrings.reconnecting) ? theme.other.buttonDisconnectProps : {};

    const showQuickConnect = !vpnConnected && cityConnectingTo === null && connectionInProgress !== ConnectingStrings.disconnecting;

    return (
        <Stack align='center' h='100vh' gap={0} style={{ backgroundImage: `url(${Deco()})`, backgroundRepeat: 'no-repeat', backgroundSize: 'contain', backgroundPosition: 'bottom' }}>
            <Space h={40} />
            <Mascot />
            <Stack align='center' gap={showQuickConnect ? 0 : 20} mt={showQuickConnect ? 0 : 20} justify='space-around'>
                <Title order={2} fw={600}>{getTitle()}</Title>
                <Title order={4} mt={5} h='xl' c={colorScheme === 'light' ? 'dark.3' : 'dark.2'} fw={350}>{Subtitle()}</Title>
            </Stack>
            <Space />
            {showQuickConnect && <Button size='md' className={commonClasses.button} onClick={qcBtnAction} w={BUTTON_WIDTH} disabled={qcBtnDisabled} {...primaryBtnDisconnectProps}>{getButtonContent()}</Button>}
            {/* quick connect cancel button */}
            {connectionInProgress === ConnectingStrings.connecting && cityConnectingTo === null && <>
                <Space h='lg' />
                <Button w={BUTTON_WIDTH} {...theme.other.buttonDisconnectProps} mt={5} onClick={vpnDisconnect}>{t('Cancel')}</Button>
            </>}
            <Space />
            {/* if quick connect is used, don't show the combobox while connecting to avoid confusion
                we do not want the user to think they are connecting to the last chosen location
                It's possible that in the future we can propagate which location is being connected to while connecting
                !(connectionInProgress === connecting && cityConnectingTo === null) */}
            {(connectionInProgress !== ConnectingStrings.connecting || cityConnectingTo !== null) &&
                <LocationConnect cityConnectingTo={cityConnectingTo} setCityConnectingTo={setCityConnectingTo} />}
            {
                vpnConnected && connectionInProgress === ConnectingStrings.UNSET && <>
                    <Space />
                    <Anchor href={CHECK_STATUS_WEBPAGE} underline='always' c={colorScheme === 'light' ? 'gray.6' : 'gray.5'}>{t('checkMyConnection')} <FaExternalLinkAlt size={12} /></Anchor>
                </>
            }
            <div style={{ flexGrow: 1 }} />
            <ConnectionProgressBar />
            <Space />
        </Stack >
    );
}

function ConnectionProgressBar() {
    const { t } = useTranslation();
    const colorScheme = useComputedColorScheme();
    const {
        vpnConnected,
        connectionInProgress,
        osStatus
    } = useContext(AppContext);
    const { internetAvailable } = osStatus;

    const bg = colorScheme === 'light' ? 'dark.9' : 'dark.6';
    const progressBg = colorScheme === 'light' ? 'dark.4' : 'dark.3';

    const connectingProgressBars = usePulsingProgress({ activated: isConnecting(connectionInProgress), bars: 2, inactiveColor: progressBg, w: 50 });

    return (
        <Paper shadow='xl' withBorder w='80%' maw={600} bg={bg} p='md' pt={5} pb='xs' mb='lg' radius='lg'>
            <Group mih={50} className={classes.connectionProgressBarGroup} align='center'>
                <Stack gap='0' align='center'>
                    <ThemeIcon variant='transparent' c='white'>
                        <MdLaptopMac size={20} />
                    </ThemeIcon>
                    <Text size='xs' c='white'>{t('Your device')}</Text>
                </Stack>
                {
                    !internetAvailable && !isConnecting(connectionInProgress) &&
                    <>
                        <Progress w={80} value={0} h={2} bg={progressBg} />
                        <Stack gap='0' align='center'>
                            <ThemeIcon variant='transparent' c='red.6'>
                                <MdOutlineWifiOff size={22} />
                            </ThemeIcon>
                            <Text size='xs' c='red'>{t('No internet connection')}</Text>
                        </Stack>
                        <Progress w={80} value={0} h={2} bg={progressBg} />
                    </>
                }
                {
                    internetAvailable && !vpnConnected && !isConnecting(connectionInProgress) &&
                    <Stack gap='xs' align='center' justify='flex-end' h={50}>
                        <Progress className={classes.trafficVulnerableProgressBar} value={100} color='red.6' h={2} bg={progressBg} />
                        <Text size='xs' c='red.6'>
                            {t('Traffic is vulnerable')}
                        </Text>
                    </Stack>
                }
                {(vpnConnected || isConnecting(connectionInProgress)) && <>
                    {connectingProgressBars[0]}
                    <Stack gap='0' align='center'>
                        <ThemeIcon variant='transparent' c='white'>
                            <Image src={ObscuraIconHappy} size={20} />
                        </ThemeIcon>
                        <Text size='xs' c='white'>Obscura</Text>
                    </Stack>
                    {connectingProgressBars[1]}
                    <Stack gap='0' align='center'>
                        <ThemeIcon variant='transparent' c={(vpnConnected && connectionInProgress !== ConnectingStrings.changingLocations) ? 'white' : 'dimmed'}>
                            <IoIosEyeOff size={20} />
                        </ThemeIcon>
                        <Text size='xs' c={(vpnConnected && connectionInProgress !== ConnectingStrings.changingLocations) ? 'white' : 'dimmed'}>Blind Relay</Text>
                    </Stack>
                    <Progress w={50} value={(vpnConnected && connectionInProgress !== ConnectingStrings.changingLocations) ? 100 : 0} h={2} bg={progressBg} />
                </>}
                <Stack gap='0' align='center'>
                    <ThemeIcon variant='transparent' c={internetAvailable ? (connectionInProgress === ConnectingStrings.UNSET ? 'white' : 'dimmed') : 'red.6'}>
                        <MdLanguage size={20} />
                    </ThemeIcon>
                    <Text size='xs' c={internetAvailable ? (connectionInProgress === ConnectingStrings.UNSET ? 'white' : 'dimmed') : 'red.6'}>{t('Internet')}</Text>
                </Stack>
            </Group>
        </Paper>
    );
}

function usePulsingProgress({ activated, bars = 2, inactiveColor, w }) {
    const activeLength = 50;
    const segmentSize = activeLength / 2;
    const values = Array.from({ length: (bars + 1) * (100 / activeLength * bars) }, (_, i) => i * segmentSize);
    // show a pause for more natural feeling
    values.push(...Array(4).fill(values.at(-1)));

    const [value, toggleValue] = useToggle(values);

    const { start, stop } = useInterval(() => {
        toggleValue();
    }, 40);

    useEffect(() => {
        if (activated) {
            start();
        } else {
            stop();
        }
        return () => stop();
    }, [activated, start, stop]);

    const progressComponents = [];

    const ProgressSection = ({ value, threshold }) => {
        return <Progress.Section bg={!activated || (value >= threshold - activeLength && value <= threshold) ? undefined : inactiveColor} value={25} />
    }

    for (let index = 0; index < bars; index++) {
        progressComponents.push(
            <Progress.Root h={2} w={w} bg={inactiveColor} transitionDuration={50}>
                <ProgressSection value={value} threshold={activeLength + 100 * index} />
                <ProgressSection value={value} threshold={(activeLength + segmentSize) + 100 * index} />
                <ProgressSection value={value} threshold={(activeLength * 2) + (100 * index)} />
                <ProgressSection value={value} threshold={(segmentSize * 5) + (100 * index)} />
            </Progress.Root>
        );
    }
    progressComponents.push(values);
    return progressComponents;
}

const DECO_CONNECTING_ARRAY = {
    light: [DecoConnectingLight1, DecoConnectingLight2, DecoConnectingLight3],
    dark: [DecoConnectingDark1, DecoConnectingDark2, DecoConnectingDark3]
};
const DEC_LAST_IDX = DECO_CONNECTING_ARRAY.light.length - 1;

function Deco() {
    const {
        vpnConnected,
        connectionInProgress,
        osStatus
    } = useContext(AppContext);
    const { internetAvailable } = osStatus;
    const colorScheme = useComputedColorScheme();

    const [connectingIndex, toggleConnectingDeco] = useToggle([0, 1, DEC_LAST_IDX, DEC_LAST_IDX]);

    // Setup interval for animation when connecting
    const { start, stop } = useInterval(() => {
        toggleConnectingDeco();
    }, 1000);

    useEffect(() => {
        if (connectionInProgress) {
            start();
        } else {
            stop();
        }
        return () => stop();
    }, [connectionInProgress, start, stop]);

    const getDecoration = () => {
        if (!internetAvailable) return colorScheme === 'light' ? DecoOfflineLight : DecoOfflineDark;

        if (connectionInProgress !== ConnectingStrings.UNSET) {
            // want to allow reverse animations
            const adjustedIdx = connectionInProgress === ConnectingStrings.disconnecting ? DEC_LAST_IDX - connectingIndex : connectingIndex;
            const connectionDeco = DECO_CONNECTING_ARRAY[colorScheme][adjustedIdx];
            if (connectionDeco === undefined) {
                console.error(`adjustedIdx/connectingIndex (${adjustedIdx} or ${connectingIndex}) longer than DECO_CONNECTING_ARRAY`);
                return DECO_CONNECTING_ARRAY[colorScheme][0];
            }
            return connectionDeco;
        };

        if (vpnConnected) return DecoConnected;
        return colorScheme === 'light' ? DecoDisconnectedLight : DecoDisconnectedDark;
    };
    return getDecoration();
}

const MASCOT_CONNECTING = [
    MascotConnecting1,
    MascotConnecting2,
    MascotConnecting3,
    MascotConnecting4
];

const ConnectedBefore = {
    NEVER: '0',
    FIRST_CONNECT: '1',
    YES: '2',
}

function Mascot() {
    const {
        vpnConnected,
        connectionInProgress,
        osStatus
    } = useContext(AppContext);
    const { internetAvailable } = osStatus;
    // tuned to show ... for 3 extra cycles
    const [connectingIndex, toggleConnectingDeco] = useToggle([0, 1, 2, 3, 3, 3, 3]);
    // want to show celebratory mascot the first time the user uses the app
    const [connectedBefore, setConnectedBefore] = useCookie('connected-before', ConnectedBefore.NEVER);

    useEffect(() => {
        if (vpnConnected) {
            if (connectedBefore === ConnectedBefore.NEVER) {
                setConnectedBefore(ConnectedBefore.FIRST_CONNECT);
            }
        } else if (!vpnConnected && connectedBefore === ConnectedBefore.FIRST_CONNECT) {
            setConnectedBefore(ConnectedBefore.YES);
        }
    }, [vpnConnected]);

    // tuned to 140ms
    const { start, stop } = useInterval(() => {
        toggleConnectingDeco();
    }, 130);

    useEffect(() => {
        if (connectionInProgress !== ConnectingStrings.UNSET) {
            start();
        } else {
            stop();
        }
        return () => stop();
    }, [connectionInProgress, start, stop]);

    const getMascot = () => {

        if (!internetAvailable) return MascotNoInternet;

        if (connectionInProgress !== ConnectingStrings.UNSET) {
            const mascotConnecting = MASCOT_CONNECTING[connectingIndex];
            if (mascotConnecting === undefined) {
                console.error(`unexpected mascot connectingIndex value ${connectingIndex}`);
                return MascotConnecting3;
            }
            return mascotConnecting;
        }
        if (vpnConnected) return connectedBefore === ConnectedBefore.FIRST_CONNECT ? MascotConnectedFirstTime : MascotConnected;
        return MascotNotConnected;
    };
    return <Image src={getMascot()} maw={90} />;
}

function LocationConnect({ cityConnectingTo, setCityConnectingTo }) {
    const { t } = useTranslation();
    const { exitList } = useContext(ExitsContext);
    const { appStatus, vpnConnect, vpnConnected, vpnDisconnectConnect, connectionInProgress, osStatus } = useContext(AppContext);
    const { internetAvailable } = osStatus;
    const { lastChosenExit, pinnedExits } = appStatus;
    const currentlyConnectedTo = appStatus?.vpnStatus.connected?.exit;
    const pinnedExitsSet = new Set(pinnedExits);

    const [opened, setOpened] = useState(false);

    const getPreferredExitId = () => {
        if (currentlyConnectedTo !== undefined) return currentlyConnectedTo.id;
        if (lastChosenExit !== null) return lastChosenExit;
        if (pinnedExits.length > 0) return pinnedExits[0];
        return null;
    }

    const [selectedExit, setSelectedExit] = useState(null);

    const setDefaultExit = () => {
        const exit = exitList.find(loc => loc.id === getPreferredExitId());
        if (exit !== undefined) {
            setSelectedExit(exit);
        }
    }

    useEffect(() => {
        // i.e. when the exitList is loaded AND (we are strictly connected or strictly disconnected)
        if (exitList !== null && connectionInProgress === ConnectingStrings.UNSET) {
            setDefaultExit();
        }
    }, [exitList, connectionInProgress]);

    // need to disable both combo (forces a collapsed dropdown) and button (non-clickable)
    const comboDisabled = !internetAvailable || connectionInProgress !== ConnectingStrings.UNSET;
    const showLastChosenLabel = lastChosenExit !== null && exitList !== null && selectedExit?.id === lastChosenExit && !vpnConnected && !isConnecting(connectionInProgress);
    const showPinned = pinnedExitsSet.has(selectedExit?.id) && (vpnConnected || isConnecting(connectionInProgress));
    return (
        <>
            <LocationConnectTopCaption cityConnectingTo={cityConnectingTo} />
            <Space />
            <Group gap='xs'>
                <Combobox
                    opened={opened}
                    onClose={() => setOpened(false)}
                    position='bottom-start'
                    withArrow={false}
                    shadow='md'
                    disabled={comboDisabled}
                    size='lg'
                >
                    <Combobox.Target>
                        <Group gap={0} style={{ minWidth: BUTTON_WIDTH }}>
                            <Button
                                disabled={comboDisabled}
                                size='lg'
                                variant='default'
                                justify='space-between'
                                onClick={() => setOpened(o => !o)}
                                flex={1}
                                rightSection={<Group gap='xs'>
                                    {showLastChosenLabel && <ObscuraChip>{t('lastChosen')}</ObscuraChip>}
                                    {showPinned && <ThemeIcon variant='transparent' size={16} c='dimmed'><BsPinFill /></ThemeIcon>}
                                    <BsChevronDown
                                        size={16}
                                        style={{
                                            transform: opened ? 'rotate(-180deg)' : null,
                                            transition: 'transform 200ms ease-in-out'
                                        }}
                                    />
                                </Group>}
                            >
                                {selectedExit === null ? <Text>{internetAvailable ? t('selectLocation') : t('noInternet')}</Text> :
                                    <Group gap='xs'>
                                        <Text size='lg'>{countryCodeToFlagEmoji(selectedExit.country_code)} {selectedExit.city_name}</Text>
                                    </Group>}
                            </Button>
                        </Group>
                    </Combobox.Target>

                    <Combobox.Dropdown>
                        <Combobox.Options>
                            <ScrollArea.Autosize type='always' mah={200} hidden={false} pt={10}>
                                <CityOptions exitList={exitList} onExitSelect={exit => {
                                    setSelectedExit(exit);
                                    setOpened(false);
                                    setCityConnectingTo(exit.city_name);
                                    if (vpnConnected) {
                                        vpnDisconnectConnect(exit.id);
                                    } else {
                                        vpnConnect(exit.id);
                                    }
                                }} lastChosenExit={lastChosenExit} pinnedExitsSet={pinnedExitsSet} />
                            </ScrollArea.Autosize>
                        </Combobox.Options>
                    </Combobox.Dropdown>
                </Combobox>
                <LocationConnectRightButton dropdownOpened={opened} selectedExit={selectedExit} setCityConnectingTo={setCityConnectingTo} />
            </Group>
        </>
    );
}

function LocationConnectTopCaption({ cityConnectingTo }) {
    const { t } = useTranslation();
    const { vpnConnected, connectionInProgress } = useContext(AppContext);
    if (vpnConnected && connectionInProgress !== ConnectingStrings.changingLocations)
        return <Text c='green.8' fw={550}>{t('connectedTo')}</Text>;

    if (connectionInProgress === ConnectingStrings.UNSET && !vpnConnected)
        return <Text c='gray'>{t('or connect to')}</Text>;

    if (isConnecting(connectionInProgress) && cityConnectingTo !== null)
        return <Text c='gray'>{t('Connecting')}...</Text>;
    return <Space h='lg' />;
}

function LocationConnectRightButton({ dropdownOpened, selectedExit, setCityConnectingTo }) {
    const { t } = useTranslation();
    const theme = useMantineTheme();
    const { vpnConnect, vpnDisconnect, vpnConnected, connectionInProgress, osStatus } = useContext(AppContext);
    const { internetAvailable } = osStatus;

    const buttonText = connectionInProgress === ConnectingStrings.connecting ? 'Cancel' : ((isConnecting(connectionInProgress) || vpnConnected) ? 'Disconnect' : 'Connect');
    const btnDisabled = (dropdownOpened && buttonText === 'Connect') || selectedExit === null || !internetAvailable || (connectionInProgress === ConnectingStrings.disconnecting || connectionInProgress === ConnectingStrings.changingLocations);
    // don't want to use color and background props when disabled since they override the disabled styles
    const disconnectVariantProps = !btnDisabled && (isConnecting(connectionInProgress) || vpnConnected) ? theme.other.buttonDisconnectProps : {};
    return (
        <Button miw={130} size='lg' fz='sm' variant='light' disabled={btnDisabled} {...disconnectVariantProps}
            onClick={() => {
                if (vpnConnected || connectionInProgress === ConnectingStrings.connecting) {
                    vpnDisconnect();
                } else {
                    setCityConnectingTo(selectedExit.city_name);
                    vpnConnect(selectedExit.id);
                }
            }}>
            {t(buttonText)}
        </Button>
    );
}

function CityOptions({ exitList, pinnedExitsSet, lastChosenExit, onExitSelect }) {
    const { t } = useTranslation();
    const [hoveredOption, setHoveredKey] = useState(null);

    if (exitList === null || exitList.size === 0) return;

    const result = [];

    const ItemRightSection = ({ exitId, hoverKey = null, showIconIfPinned = false }) => {
        // would normally use one line returns, but a mix of logic and JSX in one line is really ugly
        if (!!hoverKey && hoveredOption === hoverKey)
            return <Text size='sm' c='gray'>{t('clickToConnect')}</Text>;

        if (lastChosenExit === exitId)
            return <ObscuraChip>{t('lastChosen')}</ObscuraChip>;

        if (showIconIfPinned && pinnedExitsSet.has(exitId))
            return <ThemeIcon variant='transparent' c='dimmed'><BsPinFill /></ThemeIcon>;
    }

    const resetHoverKey = itemKey => {
        setHoveredKey(value => {
            // avoid any render race condition (confirmed it's possible without this check)
            if (value === itemKey) return null;
            return value;
        })
    }

    const getMouseHoverProps = (itemKey) => {
        return { onMouseEnter: () => setHoveredKey(itemKey), onMouseLeave: () => resetHoverKey(itemKey) };
    }

    // usually we'd conditionally render, however with the continent headings being optional, I decided
    //  to just push everything to a list. The alternative is returning <>{pinnedExits.length > 0 && {pinnedExits.maps(...)} }{result}</>
    const pinnedExits = exitList.filter(exit => pinnedExitsSet.has(exit.id));
    if (pinnedExits.length > 0) {
        result.push(<Text key='pinned-heading' size='sm' c='gray' ml='md' fw={400}><BsPinFill size={11} /> {t('Pinned')}</Text>);
        for (const exit of pinnedExits) {
            const key = `pinned.${exit.id}`;
            result.push(
                <Combobox.Option
                    className={classes.fixedHoverColor}
                    key={key}
                    value={exit.id}
                    onClick={() => onExitSelect(exit)}
                    {...getMouseHoverProps(key)}>
                    <Group gap='xs' justify='space-between'>
                        <Text size='lg'>{countryCodeToFlagEmoji(exit.country_code)} {exit.city_name}</Text>
                        <ItemRightSection exitId={exit.id} hoverKey={key} />
                    </Group >
                </Combobox.Option >
            )
        }
        result.push(<Divider key='divider-pinned' my={10} />);
    }

    const insertedContinents = new Set();

    exitList.sort(exitsSortComparator(null, null, []));

    for (const exit of exitList) {
        const continent = countries[exit.country_code.toUpperCase()].continent;
        if (!insertedContinents.has(continent)) {
            if (insertedContinents.size > 0) {
                result.push(<Divider key={`divider-${continent}`} my={10} />);
            }
            result.push(<Text key={`continent-${continent}`} size='sm' c='gray' ml='sm' fw={400}>{continents[continent]}</Text>);
            insertedContinents.add(continent);
        }
        const key = exit.id;

        result.push(
            <Combobox.Option
                className={classes.fixedHoverColor}
                key={key}
                value={exit.id}
                onClick={() => onExitSelect(exit)}
                {...getMouseHoverProps(key)}>
                <Group gap='xs' justify='space-between'>
                    <Text size='lg'>{countryCodeToFlagEmoji(exit.country_code)} {exit.city_name}</Text>
                    <ItemRightSection exitId={exit.id} hoverKey={key} showIconIfPinned />
                </Group >
            </Combobox.Option >
        )
    }
    return result;
}

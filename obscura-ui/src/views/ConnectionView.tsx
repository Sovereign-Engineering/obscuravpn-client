import { Anchor, Button, Combobox, DefaultMantineColor, Divider, Group, Image, Paper, Progress, ScrollArea, Space, Stack, StyleProp, Text, ThemeIcon, Title, useCombobox, useComputedColorScheme, useMantineTheme } from '@mantine/core';
import { useFocusTrap, useInterval, useToggle } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import { ReactNode, useContext, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BsChevronDown, BsPinFill } from 'react-icons/bs';
import { IoIosEyeOff } from 'react-icons/io';
import { MdLanguage, MdLaptopMac, MdOutlineWifiOff } from 'react-icons/md';
import { ExitSelector, ExitSelectorCity } from 'src/bridge/commands';
import * as ObscuraAccount from '../common/accountUtils';
import { accountIsExpired, Exit, getContinent, getExitCountry, useReRenderWhenExpired } from '../common/api';
import { AppContext, ConnectionInProgress, getTunnelArgs, isConnecting, PinnedLocation } from '../common/appContext';
import commonClasses from '../common/common.module.css';
import { exitLocation, exitsSortComparator, getCountryFlag, getExitCountryFlag } from '../common/exitUtils';
import { KeyedSet } from '../common/KeyedSet';
import { useExitList } from '../common/useExitList';
import { errMsg, useCookie } from '../common/utils';
import BoltBadgeAuto from '../components/BoltBadgeAuto';
import ExternalLinkIcon from '../components/ExternalLinkIcon';
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
import MascotDead from '../res/mascots/dead-mascot.svg';
import MascotNotConnected from '../res/mascots/not-connected-mascot.svg';
import MascotValidating from '../res/mascots/validating-mascot.svg';
import ObscuraIconHappy from '../res/obscura-icon-happy.svg';
import classes from './ConnectionView.module.css';

// Los Angeles, CA
const BUTTON_WIDTH = 320;

export default function Connection() {
    const theme = useMantineTheme();
    const colorScheme = useComputedColorScheme();
    const { t } = useTranslation();
    const { vpnConnected, connectionInProgress, osStatus, vpnConnect, vpnDisconnect, appStatus,  } = useContext(AppContext);
    const { internetAvailable } = osStatus;
    const connectionTransition = connectionInProgress !== ConnectionInProgress.UNSET;
    const { account } = appStatus;
    const { exitList } = useExitList({
        periodS: 3600,
    });
    const accountInfo = account?.account_info ?? null;

    useReRenderWhenExpired(account);

    const tunnelArgs = getTunnelArgs(appStatus.vpnStatus);
    const accountHasExpired = accountInfo !== null && accountIsExpired(accountInfo);

    const getTitle = () => {
        if (accountHasExpired) return t('account-Expired');
        if (connectionTransition) {
            let targetCity = tunnelArgs?.exit && "city" in tunnelArgs.exit ? tunnelArgs.exit.city : undefined;
            let sampleExit = targetCity && exitList?.find(e => e.city_code == targetCity.city_code && e.country_code == targetCity.country_code);

            switch (connectionInProgress) {
                case ConnectionInProgress.Connecting:
                case ConnectionInProgress.ChangingLocations:
                    return t('connectingTo', { location: sampleExit?.city_name ?? 'Obscura' });
                case ConnectionInProgress.Reconnecting:
                    return t('reconnectingObscura');
                case ConnectionInProgress.Disconnecting:
                    return t('Disconnecting');
            }
        }
        if (vpnConnected) return t('connectedToObscura');
        if (!internetAvailable) return t('disconnected');
        if (accountInfo === null) return t('validatingAccount')
        return t('notConnected');
    }

    const Subtitle = () => {
        if (vpnConnected && !connectionTransition) return t('enjoyObscura');
        if (accountHasExpired) return t('continueUsingObscura');
        if (connectionTransition) return t('pleaseWaitAMoment');
        if (!internetAvailable) return t('connectToInternet');
        if (accountInfo === null) return '';
        return t('connectToEnjoy');
    }

    const getButtonContent = () => {
        if (connectionTransition) return t(connectionInProgress) + '...'
        return <Group gap={5}><BoltBadgeAuto />{t('QuickConnect')}</Group>;
    }

    // qc: Quick Connect
    const qcBtnAction = (_: MouseEvent) => vpnConnected ? vpnDisconnect() : vpnConnect({ any: {} });
    const qcBtnDisabled = !internetAvailable || connectionTransition;
    const primaryBtnDisconnectProps = (vpnConnected && connectionInProgress !== ConnectionInProgress.Reconnecting) ? theme.other.buttonDisconnectProps : {};

    const isCityConnect = tunnelArgs && "city" in tunnelArgs.exit;
    const showQuickConnect = !vpnConnected && !isCityConnect && accountInfo !== null && !appStatus.vpnStatus.connecting;
    /* If quick connect is used, don't show the combobox while connecting to avoid confusion.
       This is because we don't want the user to think they are connecting to the last chosen location.
       It's possible in the future that we can propagate which location is being connected to while connecting.
       Additionally, we don't want to show the dropdown when not connected AND a connection pre-requisite is missing.
       If we're already connected and the account expires, we still want to show the disconnect.
       Only when we're no longer connected should the pre-requisite take precedence.
    */
    const showLocationSelect = (!appStatus.vpnStatus.connecting || isCityConnect) && (vpnConnected || accountInfo !== null && !accountHasExpired);

    return (
        <Stack align='center' h='100vh' gap={0} className={classes.container} style={{ backgroundImage: `url("${Deco()}")`}}>
            <Space h={40} />
            <Mascot />
            <Stack align='center' gap={showQuickConnect ? 0 : 20} mt={showQuickConnect ? 0 : 20} justify='space-around'>
                <Title order={2} fw={600}>{getTitle()}</Title>
                <Title order={4} mt={5} h='xl' c={colorScheme === 'light' ? 'dark.3' : 'dark.2'} fw={350}>{Subtitle()}</Title>
            </Stack>
            <Space h='xs' />
            {
              !vpnConnected && accountInfo !== null && accountHasExpired ?
                <Button component='a' href={ObscuraAccount.APP_ACCOUNT_TAB}>{t('ManageAccount')}</Button>
              : showQuickConnect &&
                <Button size='md' className={commonClasses.button} onClick={qcBtnAction} w={BUTTON_WIDTH} disabled={qcBtnDisabled} {...primaryBtnDisconnectProps}>{getButtonContent()}</Button>
            }
            {/* quick connect cancel button */}
            {isConnecting(connectionInProgress) && <>
                <Space h='lg' />
                <Button w={BUTTON_WIDTH} {...theme.other.buttonDisconnectProps} mt={5} onClick={vpnDisconnect}>{t('Cancel Connecting')}</Button>
            </>}
            <Space />
            {showLocationSelect && <LocationSelect />}
            {
                vpnConnected && connectionInProgress === ConnectionInProgress.UNSET && <>
                    <Space />
                    <Text c={colorScheme === 'light' ? 'gray.6' : 'gray.5'}><Anchor c='inherit' href={ObscuraAccount.CHECK_STATUS_WEBPAGE} underline='always'>{t('checkMyConnection')}</Anchor> <ExternalLinkIcon size={12} style={{ marginBottom: -1 }} /></Text>
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
    const isOffline = !internetAvailable && !vpnConnected && !isConnecting(connectionInProgress);
    return (
        <Paper shadow='xl' withBorder w='80%' maw={600} bg={bg} p='md' pt={5} pb='xs' mb='lg' radius='lg'>
            <Group mih={50} className={classes.connectionProgressBarGroup} align='center'>
                <Stack gap='0' align='center'>
                    <ThemeIcon variant='transparent' c='white'>
                        <MdLaptopMac size={20} />
                    </ThemeIcon>
                    <Text size='xs' c='white'>{t('yourDevice')}</Text>
                </Stack>
                {
                    isOffline &&
                    <>
                        <Progress w={80} value={0} h={2} bg={progressBg} />
                        <Stack gap='0' align='center'>
                            <ThemeIcon variant='transparent' c='red.6'>
                                <MdOutlineWifiOff size={22} />
                            </ThemeIcon>
                            <Text size='xs' c='red'>{t('noInternet')}</Text>
                        </Stack>
                        <Progress w={80} value={0} h={2} bg={progressBg} />
                    </>
                }
                {
                    internetAvailable && (connectionInProgress === ConnectionInProgress.Disconnecting || (!vpnConnected && connectionInProgress === ConnectionInProgress.UNSET)) &&
                    <Stack gap='xs' align='center' justify='flex-end' h={50}>
                        <Progress className={classes.trafficVulnerableProgressBar} value={100} color='red.6' h={2} bg={progressBg} />
                        <Text size='xs' c='red.6'>
                            {t('trafficVulnerable')}
                        </Text>
                    </Stack>
                }
                {
                    ((vpnConnected && connectionInProgress === ConnectionInProgress.UNSET) || isConnecting(connectionInProgress)) && <>
                    {connectingProgressBars[0]}
                    <Stack gap='0' align='center'>
                        <ThemeIcon variant='transparent' c='white'>
                            <Image src={ObscuraIconHappy} w={20} />
                        </ThemeIcon>
                        <Text size='xs' c='white'>Obscura</Text>
                    </Stack>
                    {connectingProgressBars[1]}
                    <Stack gap='0' align='center'>
                        <ThemeIcon variant='transparent' c={(vpnConnected && connectionInProgress !== ConnectionInProgress.ChangingLocations) ? 'white' : 'dimmed'}>
                            <IoIosEyeOff size={20} />
                        </ThemeIcon>
                        <Text size='xs' c={(vpnConnected && connectionInProgress !== ConnectionInProgress.ChangingLocations) ? 'white' : 'dimmed'}>Blind Relay</Text>
                    </Stack>
                    <Progress w={50} value={(vpnConnected && connectionInProgress !== ConnectionInProgress.ChangingLocations) ? 100 : 0} h={2} bg={progressBg} />
                </>}
                <Stack gap='0' align='center'>
                    <ThemeIcon variant='transparent' c={isOffline ? 'red.6' : (connectionInProgress === ConnectionInProgress.UNSET ? 'white' : 'dimmed')}>
                        <MdLanguage size={20} />
                    </ThemeIcon>
                    <Text size='xs' c={isOffline ? 'red.6' : (connectionInProgress === ConnectionInProgress.UNSET ? 'white' : 'dimmed')}>{t('Internet')}</Text>
                </Stack>
            </Group>
        </Paper>
    );
}

interface PulsingProgressProps {
  activated: boolean,
  bars: number,
  inactiveColor: StyleProp<DefaultMantineColor>,
  w: number
}

function usePulsingProgress({ activated, bars = 2, inactiveColor, w }: PulsingProgressProps) {
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

    const progressComponents: ReactNode[] = [];

    const ProgressSection = ({ value, threshold }: { value: number, threshold: number }) => {
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
    const [connectingIndex, toggleConnectingDeco] = useToggle([0, 1, 2, 2]);

    // Setup interval for animation when connecting
    const { start, stop } = useInterval(() => {
      toggleConnectingDeco();
    }, 1000);

    useEffect(() => {
        if (connectionInProgress !== ConnectionInProgress.UNSET) {
            start();
        } else {
            if (connectingIndex !== 0) {
              toggleConnectingDeco(0);
            }
            stop();
        }
        return () => stop();
    }, [connectionInProgress, start, stop]);

    const isOffline = !internetAvailable && !vpnConnected && !isConnecting(connectionInProgress);
    if (isOffline) return colorScheme === 'light' ? DecoOfflineLight : DecoOfflineDark;

    if (connectionInProgress !== ConnectionInProgress.UNSET) {
        // want to allow reverse animations
        const adjustedIdx = connectionInProgress === ConnectionInProgress.Disconnecting ? DEC_LAST_IDX - connectingIndex : connectingIndex;
        const connectionDeco = DECO_CONNECTING_ARRAY[colorScheme][adjustedIdx];
        if (connectionDeco === undefined) {
            console.error(`adjustedIdx/connectingIndex (${adjustedIdx} or ${connectingIndex}) longer than DECO_CONNECTING_ARRAY`);
            return DECO_CONNECTING_ARRAY[colorScheme][0];
        }
        return connectionDeco;
    };

    if (vpnConnected) return DecoConnected;
    return colorScheme === 'light' ? DecoDisconnectedLight : DecoDisconnectedDark;
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
        osStatus,
        appStatus
    } = useContext(AppContext);
    const accountInfo = appStatus.account?.account_info ?? null;
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
        if (connectionInProgress !== ConnectionInProgress.UNSET) {
            start();
        } else {
            if (connectingIndex !== 0) {
              toggleConnectingDeco(0);
            }
            stop();
        }
        return () => stop();
    }, [connectionInProgress, start, stop]);

    const isOffline = !internetAvailable && !vpnConnected && !isConnecting(connectionInProgress);

    const getMascot = () => {
        if (isOffline) return MascotDead;
        if (connectionInProgress !== ConnectionInProgress.UNSET) {
            const mascotConnecting = MASCOT_CONNECTING[connectingIndex];
            if (mascotConnecting === undefined) {
                console.error(`unexpected mascot connectingIndex value ${connectingIndex}`);
                return MascotConnecting3;
            }
            return mascotConnecting;
        }
        if (vpnConnected) return connectedBefore === ConnectedBefore.FIRST_CONNECT ? MascotConnectedFirstTime : MascotConnected;
        if (accountInfo === null) return MascotValidating;
        if (accountIsExpired(accountInfo)) return MascotDead;
        return MascotNotConnected;
    };
    return <Image src={getMascot()} maw={90} />;
}

function LocationSelect(): ReactNode {
    const { t } = useTranslation();

    const { exitList } = useExitList({
        periodS: 60,
    });
    const locations = useMemo(() => {
      return new KeyedSet(
        (exit: {country_code: string, city_code: string}) => JSON.stringify([exit.country_code, exit.city_code]),
        exitList,
      );
    }, [exitList]);

    const { appStatus, vpnConnect, vpnConnected, connectionInProgress, osStatus } = useContext(AppContext);
    const { internetAvailable } = osStatus;
    const { lastChosenExit, pinnedLocations } = appStatus;
    const connectedExit = appStatus.vpnStatus.connected?.exit;
    const pinnedLocationSet = new KeyedSet(
      (loc: {city_code: string, country_code: string}) => JSON.stringify([loc.country_code, loc.city_code]),
      pinnedLocations,
    );

    const [selectedCity, setSelectedCity] = useState<ExitSelectorCity | null>(null);
    const selectedExampleExit = selectedCity && locations.get(selectedCity);

    useEffect(() => {
      if (connectionInProgress !== ConnectionInProgress.UNSET) {
        return;
      }

      if (connectedExit !== undefined) {
        setSelectedCity({
          city_code: connectedExit.city_code,
          country_code: connectedExit.country_code,
        });
      } else if (lastChosenExit && "city" in lastChosenExit) {
        setSelectedCity(lastChosenExit.city);
      } else if (pinnedLocations.length > 0) {
        let loc = pinnedLocations[0]!;
        setSelectedCity({
          city_code: loc.city_code,
          country_code: loc.country_code,
        });
      }
    }, [connectionInProgress]);

    // need to disable both combo (forces a collapsed dropdown) and button (non-clickable)
    const comboDisabled = !internetAvailable || connectionInProgress !== ConnectionInProgress.UNSET;
    const showLastChosenLabel = lastChosenExit
      && "city" in lastChosenExit
      && selectedCity?.city_code === lastChosenExit.city.city_code
      && selectedCity?.country_code === lastChosenExit.city.country_code
      && !!exitList
      && !vpnConnected
      && !isConnecting(connectionInProgress);
    const showPinned = selectedCity !== null
      && pinnedLocationSet.has(selectedCity)
      && (vpnConnected || isConnecting(connectionInProgress));

    const combobox = useCombobox({
        onDropdownClose: () => combobox.resetSelectedOption(),
    });
    const focusRef = useFocusTrap(combobox.dropdownOpened);

    return (
        <>
            <LocationConnectTopCaption />
            <Space />
            <Group gap='xs'>
                <Combobox
                    store={combobox}
                    position='bottom-start'
                    withArrow={false}
                    shadow='md'
                    disabled={comboDisabled}
                    size='lg'
                >
                    <Combobox.Target>
                        <Group ref={focusRef} gap={0} style={{ minWidth: BUTTON_WIDTH }}>
                            <Button
                                disabled={comboDisabled}
                                size='lg'
                                variant='default'
                                justify='space-between'
                                onClick={() => combobox.toggleDropdown()}
                                flex={1}
                                rightSection={<Group gap='xs'>
                                    {showLastChosenLabel && <ObscuraChip>{t('lastChosen')}</ObscuraChip>}
                                    {showPinned && <ThemeIcon variant='transparent' size={16} c='dimmed'><BsPinFill /></ThemeIcon>}
                                    <BsChevronDown
                                        size={16}
                                        style={{
                                            transform: combobox.dropdownOpened ? 'rotate(-180deg)' : undefined,
                                            transition: 'transform 200ms ease-in-out'
                                        }}
                                    />
                                </Group>}
                            >
                                {
                                  selectedCity === null
                                  ? <Text>{internetAvailable ? t('selectLocation') : t('noInternet')}</Text>
                                  : <Group gap='xs'>
                                    <Text size='lg'>{getCountryFlag(selectedCity.country_code)} {selectedExampleExit?.city_name}</Text>
                                  </Group>
                                }
                            </Button>
                        </Group>
                    </Combobox.Target>

                    <Combobox.Dropdown>
                        <Combobox.Options>
                            <ScrollArea.Autosize type='always' mah={200} hidden={false} pt={10}>
                                <CityOptions locations={locations} onExitSelect={(country_code: string, city_code: string) => {
                                    combobox.closeDropdown();
                                    if (connectedExit?.country_code === country_code && connectedExit?.city_code === city_code) {
                                      return;
                                    }
                                    let location = locations.get({
                                        country_code,
                                        city_code,
                                    });
                                    if (!location) {
                                        console.error("No exit matching selected location", country_code, city_code);
                                        notifications.show({
                                            title: t('Error'),
                                            message: t('InternalError'),
                                            color: 'red',
                                        });
                                        return;
                                    }
                                    let city = {
                                        country_code,
                                        city_code,
                                    };
                                    setSelectedCity(city);
                                    vpnConnect({
                                      city: city,
                                    });
                                }} lastChosenExit={lastChosenExit} pinnedLocationSet={pinnedLocationSet} />
                            </ScrollArea.Autosize>
                        </Combobox.Options>
                    </Combobox.Dropdown>
                </Combobox>
                <LocationConnectRightButton dropdownOpened={combobox.dropdownOpened} selectedCity={selectedCity} />
            </Group>
        </>
    );
}

function LocationConnectTopCaption() {
    const { t } = useTranslation();
    const { vpnConnected, connectionInProgress } = useContext(AppContext);
    if (vpnConnected && connectionInProgress === ConnectionInProgress.UNSET)
        return <Text c='green.8' fw={550}>{t('connectedTo')}</Text>;

    if (connectionInProgress === ConnectionInProgress.UNSET && !vpnConnected)
        return <Text c='gray'>{t('or connect to')}</Text>;

    if (isConnecting(connectionInProgress))
        return <Text c='gray'>{t('Connecting')}...</Text>;
    return <Space h='lg' />;
}

interface LocationConnectRightButtonProps {
  dropdownOpened: boolean,
  selectedCity: ExitSelectorCity | null,
}

function LocationConnectRightButton({ dropdownOpened, selectedCity }: LocationConnectRightButtonProps) {
    const { t } = useTranslation();
    const theme = useMantineTheme();
    const { vpnConnect, vpnDisconnect, vpnConnected, connectionInProgress, osStatus } = useContext(AppContext);
    const { internetAvailable } = osStatus;

    const buttonText = connectionInProgress === ConnectionInProgress.Connecting ? 'Cancel' : ((isConnecting(connectionInProgress) || vpnConnected) ? 'Disconnect' : 'Connect');
    const btnDisabled = (dropdownOpened && buttonText === 'Connect') || selectedCity === null || !internetAvailable || (connectionInProgress === ConnectionInProgress.Disconnecting || connectionInProgress === ConnectionInProgress.ChangingLocations);
    // don't want to use color and background props when disabled since they override the disabled styles
    const disconnectVariantProps = !btnDisabled && (isConnecting(connectionInProgress) || vpnConnected) ? theme.other.buttonDisconnectProps : {};
    return (
        <Button miw={130} size='lg' fz='sm' variant='light' disabled={btnDisabled} {...disconnectVariantProps}
            onClick={() => {
                if (vpnConnected || connectionInProgress === ConnectionInProgress.Connecting) {
                    vpnDisconnect();
                } else if (selectedCity !== null) {
                    vpnConnect({
                      city: selectedCity,
                    });
                }
            }}>
            {t(buttonText)}
        </Button>
    );
}

interface CityOptionsProps {
  locations: KeyedSet<Exit>,
  pinnedLocationSet: KeyedSet<PinnedLocation>,
  lastChosenExit: ExitSelector | undefined,
  onExitSelect: (country_code: string, city_code: string) => void
}

interface ItemRightSectionProps {
    exit: Exit,
    hoverKey: string,
    showIconIfPinned?: boolean
}

function CityOptions({ locations, pinnedLocationSet, lastChosenExit, onExitSelect }: CityOptionsProps) {
    const { t } = useTranslation();
    const [hoveredOption, setHoveredKey] = useState<string | null>(null);

    if (locations.size === 0) return;

    let lastCity = lastChosenExit && "city" in lastChosenExit && lastChosenExit.city || undefined;

    const ItemRightSection = ({ exit, hoverKey, showIconIfPinned = false }: ItemRightSectionProps) => {
        // would normally use one line returns, but a mix of logic and JSX in one line is really ugly
        if (!!hoverKey && hoveredOption === hoverKey)
            return <Text size='sm' c='gray'>{t('clickToConnect')}</Text>;

        if (lastCity?.city_code === exit.city_code && lastCity?.country_code === exit.country_code)
            return <ObscuraChip>{t('lastChosen')}</ObscuraChip>;

        if (showIconIfPinned && pinnedLocationSet.has(exitLocation(exit)))
            return <ThemeIcon variant='transparent' c='dimmed'><BsPinFill /></ThemeIcon>;
    }

    const resetHoverKey = (itemKey: string) => {
        setHoveredKey(value => {
            // avoid any render race condition (confirmed it's possible without this check)
            if (value === itemKey) return null;
            return value;
        })
    }

    const getMouseHoverProps = (itemKey: string) => {
        return { onMouseEnter: () => setHoveredKey(itemKey), onMouseLeave: () => resetHoverKey(itemKey) };
    }

    let orderedLocations = [...locations];
    orderedLocations.sort(exitsSortComparator);

    const result: ReactNode[] = [];

    for (const exit of orderedLocations) {
      if (!pinnedLocationSet.has(exitLocation(exit))) continue;

      if (result.length === 0) {
        result.push(<Text key='pinned-heading' size='sm' c='gray' ml='md' fw={400}><BsPinFill size={11} /> {t('Pinned')}</Text>);
      }

      const key = JSON.stringify(['pinned', exit.country_code, exit.city_code]);
      result.push(
        <Combobox.Option
          className={classes.fixedHoverColor}
          key={key}
          value={key}
          onClick={() => onExitSelect(exit.country_code, exit.city_code)}
          {...getMouseHoverProps(key)}>
          <Group gap='xs' justify='space-between'>
            <Text size='lg'>{getExitCountryFlag(exit)} {exit.city_name}</Text>
            <ItemRightSection exit={exit} hoverKey={key} />
          </Group >
        </Combobox.Option >
      );
    }
    if (result.length > 0) {
      result.push(<Divider key='divider-pinned' my={10} />);
    }

    const insertedContinents = new Set();

    for (const exit of orderedLocations) {
      const continent = getContinent(getExitCountry(exit));
      if (!insertedContinents.has(continent)) {
        if (insertedContinents.size > 0) {
          result.push(<Divider key={`divider-${continent}`} my={10} />);
        }
        insertedContinents.add(continent);
        result.push(<Text key={`continent-${continent}`} size='sm' c='gray' ml='sm' fw={400}>{t(`Continent${continent}`)}</Text>);
      }
      const key = JSON.stringify([exit.country_code, exit.city_code]);

      result.push(
        <Combobox.Option
          className={classes.fixedHoverColor}
          key={key}
          value={key}
          onClick={() => onExitSelect(exit.country_code, exit.city_code)}
          {...getMouseHoverProps(key)}>
          <Group gap='xs' justify='space-between'>
              <Text size='lg'>{getExitCountryFlag(exit)} {exit.city_name}</Text>
              <ItemRightSection exit={exit} hoverKey={key} showIconIfPinned />
          </Group >
        </Combobox.Option >
      );
    }

    return result;
}

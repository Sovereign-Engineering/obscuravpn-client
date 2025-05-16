import { Accordion, Autocomplete, Button, Group, JsonInput, Stack, Text, TextInput, Title } from '@mantine/core';
import { useInterval } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import Cookies from 'js-cookie';
import { useContext, useEffect, useRef, useState } from 'react';
import * as commands from '../bridge/commands';
import { PLATFORM } from '../bridge/SystemProvider';
import { AppContext } from '../common/appContext';
import { localStorageGet, LocalStorageKey } from '../common/localStorage';
import { IS_WK_WEB_VIEW } from '../common/utils';
import DevSendCommand from '../components/DevSendCommand';
import DevSetApiUrl from '../components/DevSetApiUrl';

export default function DeveloperViewer() {
    const { vpnConnected, connectionInProgress, appStatus, osStatus } = useContext(AppContext);
    const [trafficStats, setTrafficStats] = useState({});
    const cookieToDeleteRef = useRef<HTMLInputElement | null>(null);

    const trafficStatsInterval = useInterval(async () => {
        setTrafficStats(await commands.getTrafficStats());
    }, 1000);

    useEffect(() => {
        trafficStatsInterval.start();
        return () => {
            trafficStatsInterval.stop();
        };
    }, []);

    const [localStorageKey, setLocalStorageKey] = useState('');
    const [localStorageValue, setLocalStorageValue] = useState<string | null>(null);

    const [accordionValues, setAccordionValues] = useState<string[]>([]);

    return <Stack p={20} mb={50}>
        <Title order={3}>Developer View</Title>
        <Title order={4}>Statuses</Title>
        <Accordion multiple value={accordionValues} onChange={setAccordionValues}>
            <Accordion.Item key='appStatus' value='appStatus'>
                <Accordion.Control>App Status</Accordion.Control>
                <Accordion.Panel>
                    <JsonInput value={JSON.stringify(appStatus, null, 4)} contentEditable={false} rows={10} />
                </Accordion.Panel>
            </Accordion.Item>
            <Accordion.Item key='osStatus' value='osStatus'>
                <Accordion.Control>OsStatus</Accordion.Control>
                <Accordion.Panel>
                    <JsonInput value={JSON.stringify(osStatus, null, 4)} contentEditable={false} rows={10} />
                </Accordion.Panel>
            </Accordion.Item>
            <Accordion.Item key='systemInfo' value='systemInfo'>
                <Accordion.Control>Build Time Info</Accordion.Control>
                <Accordion.Panel>
                    PLATFORM = {PLATFORM}
                </Accordion.Panel>
            </Accordion.Item>
        </Accordion>
        <Title order={4}>React variables</Title>
        <Text>vpn is connected: <b>{vpnConnected ? 'Yes' : 'No'}</b></Text>
        <Text>connection in progress: <b>{connectionInProgress ?? 'No'}</b></Text>
        {IS_WK_WEB_VIEW && <><Button title='Preferences such as whether the user has been onboarded or if the app has tried to register as a login item' onClick={() => commands.developerResetUserDefaults().then(() => notifications.show({ title: 'Successfully Removed UserDefault Keys', color: 'green', message: '' }))}>Reset app UserDefaults</Button></>}
        <DevSetApiUrl />
        <Title order={4}>Traffic Stats</Title>
        <Text>Since this is cumulative, to get the average bandwidth speed, you must do a slope calculation between the time of two captures (recommended gap of 500ms to 1000ms). See code in the <code>apple/client/StatusItem</code> directory for reference</Text>
        <JsonInput value={JSON.stringify(trafficStats, null, 4)} contentEditable={false} rows={6} />
        <Title order={4}>Exit Servers</Title>
        <DevSendCommand />
        <Button onClick={() => commands.setInNewAccountFlow(true)}>setInNewAccountFlow</Button>
        <Title order={4}>Cookies</Title>
        <Text>{JSON.stringify(Cookies.get(), null, 4)}</Text>
        <Group>
            <TextInput ref={cookieToDeleteRef} placeholder='cookieName' />
            <Button onClick={() => {
                if (cookieToDeleteRef.current !== null) Cookies.remove(cookieToDeleteRef.current.value)
            }}>Delete Cookie</Button>
        </Group>
        <Title order={4}>Local Storage</Title>
        <Group align='end'>
          <Autocomplete onChange={setLocalStorageKey} label='local storage key' data={Object.values(LocalStorageKey)} />
          <Button onClick={() => setLocalStorageValue(localStorageGet(localStorageKey as LocalStorageKey))}>Get</Button>
        </Group>
        <JsonInput value={localStorageValue ?? 'null'} contentEditable={false} />
    </Stack>;
}

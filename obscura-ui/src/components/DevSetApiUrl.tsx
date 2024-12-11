import { Code, Stack, Title } from '@mantine/core';
import { useContext, useState } from 'react';
import { jsonFfiCmd } from "../bridge/commands";
import { AppContext } from '../common/appContext';
import { localStorageGet, LocalStorageKey, localStorageSet } from '../common/localStorage';
import { SelectCreatable } from './SelectCreatable';

const defaultApiUrls = ['https://v1.api.prod.obscura.net/api', 'https://v1.api.staging.obscura.net/api', 'http://localhost:8080/api']

export default function DevSetApiUrl() {
  let [output, setOutput] = useState('');
  let apiUrls = [...defaultApiUrls];
  let lastCustomApiUrl = localStorageGet(LocalStorageKey.LastCustomApiUrl);
  if (lastCustomApiUrl !== null && !defaultApiUrls.includes(lastCustomApiUrl)) {
    apiUrls.push(lastCustomApiUrl);
  }
  let [apiUrlOptions, setApiUrlOptions] = useState(apiUrls);
  const { appStatus } = useContext(AppContext);

  const onSubmit = (url: string | null) => {
    setOutput('');
    (async () => {
      try {
        if (url === '') {
          url = null;
        }
        if (url !== null && !defaultApiUrls.includes(url)) {
          localStorageSet(LocalStorageKey.LastCustomApiUrl, url);
          setApiUrlOptions([...defaultApiUrls, url]);
        }
        await jsonFfiCmd("setApiUrl", { url });
      } catch (e) {
        setOutput(`${e}`);
      }
    })()
  }

  return <>
    <Title order={4}>Set Backend URL</Title>
    <Stack gap={0}>
      <SelectCreatable defaultValue={appStatus.apiUrl} choices={apiUrlOptions} onSubmit={onSubmit} inputBaseProps={{ type: 'url' }} />
      {output && <Code block c='red.6' style={{ whiteSpace: 'pre-wrap' }}>{output}</Code>}
    </Stack>
  </>;
}

import { Code, Stack, Title } from '@mantine/core';
import { useContext, useState } from 'react';
import { jsonFfiCmd, setApiUrl } from "../bridge/commands";
import { AppContext } from '../common/appContext';
import { getCustomApiUrls, setCustomApiUrls } from '../common/localStorage';
import { Choice, SelectCreatable } from './SelectCreatable';

const defaultApiUrls = new Set(['https://v1.api.prod.obscura.net/api', 'https://v1.api.staging.obscura.net/api', 'http://localhost:8080/api', '']);

export default function DevSetApiUrl() {
  let [output, setOutput] = useState('');
  let apiUrls = [...defaultApiUrls.values()];
  const customApiUrls = new Set(getCustomApiUrls());

  for (const customApiUrl of customApiUrls) {
    if (!defaultApiUrls.has(customApiUrl) && !apiUrls.includes(customApiUrl)) {
      apiUrls.push(customApiUrl);
    }
  }

  const initialApiUrlOptions: Choice[] = apiUrls.map(value => ({ text: value === '' ? 'null' : value, value }));
  let [apiUrlOptions, setApiUrlOptions] = useState(initialApiUrlOptions);
  const { appStatus } = useContext(AppContext);

  const onSubmit = (url: string | null) => {
    setOutput('');
    (async () => {
      try {
        if (url === '') {
          url = null;
        }
        // add new urls to custom api urls
        if (url !== null && !defaultApiUrls.has(url) && !customApiUrls.has(url)) {
          setCustomApiUrls([url, ...customApiUrls]);
          setApiUrlOptions([{ value: url, text: url }, ...apiUrlOptions]);
        }
        await setApiUrl(url);
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

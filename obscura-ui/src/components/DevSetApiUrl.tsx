import { Autocomplete, Button, Title } from '@mantine/core';
import { useRef, useState } from 'react';
import { jsonFfiCmd } from "../bridge/commands";
import { localStorageGet, LocalStorageKey, localStorageSet } from '../common/localStorage';

const defaultApiUrls = ['https://v1.api.prod.obscura.net/api', 'https://v1.api.staging.obscura.net/api', 'http://localhost:8080/api']

export default function DevSetApiUrl() {
    let [output, setOutput] = useState("");
    let inputRef = useRef<HTMLInputElement>(null);

    let apiUrls = [...defaultApiUrls];
    let lastCustomApiUrl = localStorageGet(LocalStorageKey.LastCustomApiUrl);
    if (lastCustomApiUrl !== null && !defaultApiUrls.includes(lastCustomApiUrl)) {
        apiUrls.push(lastCustomApiUrl);
    }
    let [apiUrlOptions, setApiUrlOptions] = useState(apiUrls);

    return <>
        <Title order={4}>Set Backend URL</Title>
        <form
            style={{ display: "grid" }}
            onKeyDown={e => {
                if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                    e.preventDefault();
                    e.currentTarget.requestSubmit();
                }
            }}
            onSubmit={e => {
                e.preventDefault();
                setOutput('');
                (async () => {
                    try {
                        let url: string | null = inputRef.current!.value
                        if (url === '') {
                            url = null;
                        } else if (!URL.canParse(inputRef.current!.value)) {
                            throw new Error("Must be valid URL.");
                        }
                        if (url !== null && !defaultApiUrls.includes(url)) {
                            let lastCustomApiUrl = localStorageSet(LocalStorageKey.LastCustomApiUrl, url);
                            setApiUrlOptions([...defaultApiUrls, url]);
                        }
                        await jsonFfiCmd("setApiUrl", { url });
                    } catch (e) {
                        setOutput(`${e}`);
                    }
                })()
            }}
        >
            <Autocomplete
                ref={inputRef}
                defaultValue={``}
                data={apiUrlOptions}
            />
            <Button type="submit">Set API URL</Button>
            {output && <input type="text" value={output} disabled />}
        </form>
    </>
}

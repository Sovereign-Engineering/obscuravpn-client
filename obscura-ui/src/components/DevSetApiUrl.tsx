import { Autocomplete, Button, TextInput, Title } from '@mantine/core';
import React, { useRef, useState } from 'react';
import { jsonFfiCmd } from "../tauri/commands";

export default function DevSetApiUrl(): React.ReactElement {
    let [output, setOutput] = useState("");
    let inputRef = useRef();

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
                        let url = inputRef.current!.value
                        if (url === '') {
                            url = null;
                        } else if (!URL.canParse(inputRef.current!.value)) {
                            throw new Error("Must be valid URL.");
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
                data={['https://v1.api.prod.obscura.net/api', 'https://v1.api.staging.obscura.net/api', 'http://localhost:8080/api']}
            />
            <Button type="submit">Set API URL</Button>
            {output && <input type="text" value={output} disabled />}
        </form>
    </>
}

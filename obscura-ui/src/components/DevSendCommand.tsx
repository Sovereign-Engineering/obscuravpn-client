import { Button, JsonInput, Title } from '@mantine/core';
import { useRef, useState } from 'react';
import { jsonFfiCmd } from "../bridge/commands";

export default function DevSendCommand() {
    let [output, setOutput] = useState("");
    let inputRef = useRef<HTMLTextAreaElement>(null);

    return <>
        <Title order={4}>Send Command</Title>
        <form
            style={{ display: "grid" }}
            onKeyDown={e => {
                if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                    e.preventDefault();
                    e.currentTarget.requestSubmit();
                }
            }}
            onSubmit={e => {
                setOutput("Running...");
                e.preventDefault();
                (async () => {
                    try {
                        let cmd = JSON.parse(inputRef.current!.value);
                        let pairs = Object.entries(cmd);
                        if (pairs.length !== 1) {
                            throw new Error("Command must have one top-level property.");
                        }
                        const [name, args] = pairs[0]!;
                        let r = await jsonFfiCmd(name, args as {});
                        setOutput(JSON.stringify(r, null, "\t"));
                    } catch (e) {
                        setOutput(`${e}`);
                    }
                })()
            }}
        >
            <JsonInput
                ref={inputRef}
                defaultValue={`{"getStatus": {}}`}
                autosize
                spellCheck={false} // Disable macOS pretty quotes.
            />
            <Button type="submit">Run</Button>
            {output && <textarea value={output} disabled rows={output.split("\n").length} />}
        </form>
    </>
}

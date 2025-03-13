export function fmt(lits: TemplateStringsArray, ...values: unknown[]): string {
    let out: unknown[] = [];
    for (let i = 0; ; i++) {
        out.push(lits[i]);

        if (i >= values.length) break;
        let v = values[i];

        let type = typeof v;
        switch (type) {
            case "bigint":
            case "boolean":
            case "number":
            case "symbol":
            case "undefined":
                out.push(v);
                break;
            case "object":
            case "string":
                if (v instanceof Error) {
                  out.push(`${v}`);
                } else {
                  out.push(JSON.stringify(v));
                }
                break;
            case "function":
                out.push(`[function ${(v as Function).name}]`);
                break;
            default:
                let _: never = type;
                void _;
                out.push(JSON.stringify(v));
        }
    }

    return out.join("");
}

class InterpolatedError extends Error {
    constructor(
        message: string,
        readonly values: unknown[],
    ) {
        super(message);
    }
}

export function err(lits: TemplateStringsArray, ...values: unknown[]): Error {
    return new InterpolatedError(fmt(lits, ...values), values);
}

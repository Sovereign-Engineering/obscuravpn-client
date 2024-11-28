import {readFileSync} from "node:fs";

function rpartition(s, p) {
    let i = s.lastIndexOf(p);
    if (i < 0) {
        throw new Error(`No ${JSON.stringify(p)} in ${JSON.stringify(s)}`)
    }
    return [
        s.slice(0, i),
        s.slice(i + 1),
    ];
}

const LICENSES_NODE = JSON.parse(readFileSync(process.env.LICENSES_NODE));
const LICENSES_RUST = JSON.parse(readFileSync(process.env.LICENSES_RUST));

let overview = new Map;
let licenses = new Map;

let out = {
    overview: [],
    licenses: [],
};

function addLicense(info) {
    let o = overview.get(info.id);
    if (!o) {
        o = {
            id: info.id,
            name: info.name,
            count: 0,
        };
        overview.set(info.id, o);
        out.overview.push(o);
    }
    o.count += 1;

    let l = licenses.get(info.text);
    if (!l) {
        l = {
            id: info.id,
            name: o.name,
            text: info.text,
            used_by: [],
        };
        licenses.set(info.text, l);
        out.licenses.push(l);
    }
    l.used_by.push(info.package);
}

// Note: Do Rust licenses first as they have nice names.
for (let license of LICENSES_RUST.licenses) {
    for (let {crate} of license.used_by) {
        addLicense({
            id: license.id,
            name: license.name,
            text: license.text,
            package: {
                name: crate.name,
                url: crate.repository ?? `https://crates.io/crates/${crate.name}`,
                version: crate.version,
            },
        });
    }
}

for (let [pkg, info] of Object.entries(LICENSES_NODE)) {
    let id = /[a-zA-Z0-9-]+/.exec(info.licenses)[0];
    let [name, version] = rpartition(pkg, "@");
    addLicense({
        id,
        name: id,
        text: info.licenseFile ? readFileSync(info.licenseFile).toString() : info.id,
        package: {
            name,
            url: info.repository ?? `https://www.npmjs.com/package/${name}`,
            version,
        },
    });
}

function cmp(l, r) {
    if (l < r) return -1;
    if (l > r) return 1;
    return 0;
}
out.overview.sort((l, r) => {
    return cmp(l.id, r.id)
        || cmp(l.name, r.name)
        || cmp(l.count, r.count);
})
out.licenses.sort((l, r) => {
    return cmp(l.id, r.id)
        || cmp(l.name, r.name)
        || cmp(l.text, r.text);
})

console.log(JSON.stringify(out, null, "\t"));

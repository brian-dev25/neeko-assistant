const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');
const version = process.argv[2];

if (!version || !/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
    console.error('Usage: npm run version:bump -- 1.0.1');
    process.exit(1);
}

function readJson(relativePath) {
    return JSON.parse(fs.readFileSync(path.join(root, relativePath), 'utf8'));
}

function writeJson(relativePath, data) {
    fs.writeFileSync(
        path.join(root, relativePath),
        `${JSON.stringify(data, null, 2)}\n`,
        'utf8'
    );
}

function replaceInFile(relativePath, replacements) {
    const filePath = path.join(root, relativePath);
    let content = fs.readFileSync(filePath, 'utf8');
    for (const [pattern, replacement] of replacements) {
        content = content.replace(pattern, replacement);
    }
    fs.writeFileSync(filePath, content, 'utf8');
}

const packageJson = readJson('package.json');
packageJson.version = version;
writeJson('package.json', packageJson);

const packageLock = readJson('package-lock.json');
packageLock.version = version;
if (packageLock.packages?.['']) {
    packageLock.packages[''].version = version;
}
writeJson('package-lock.json', packageLock);

replaceInFile('src-tauri/Cargo.toml', [
    [/^version = ".+"$/m, `version = "${version}"`],
]);

replaceInFile('src-tauri/Cargo.lock', [
    [
        /(\[\[package\]\]\r?\nname = "neeko-assistant"\r?\nversion = )".+"/,
        `$1"${version}"`,
    ],
]);

replaceInFile('src-tauri/tauri.conf.json', [
    [/"version":\s*".+"/, `"version": "${version}"`],
]);

replaceInFile('web/index.html', [
    [/v\d+\.\d+\.\d+/g, `v${version}`],
]);

console.log(`Version bumped to ${version}`);

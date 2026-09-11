import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
import { mkdirSync, readFileSync, readdirSync, existsSync, copyFileSync } from 'node:fs';

if (process.platform === 'win32') {
    const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
    const output = resolve(root, 'src-tauri/binaries/screen-translate');
    mkdirSync(output, { recursive: true });
    const result = spawnSync('dotnet', ['publish', resolve(root, 'native/screen-translate/Neeko.ScreenOcr.csproj'),
        '-c', 'Release', '-r', 'win-x64', '--self-contained', 'true', '-o', output, '--nologo'],
    { stdio: 'inherit', windowsHide: true });
    if (result.error) console.error('Screen Translate requiere .NET SDK 9 para compilar el motor:', result.error.message);
    if (result.status !== 0) process.exit(result.status ?? 1);
    copyFileSync(resolve(root, 'native/screen-translate/LICENSE-MORT.txt'), resolve(output, 'LICENSE-MORT.txt'));
    // Self-contained publishing must also ship the runtime's redistribution notices.
    // Resolve the exact restored packs instead of assuming the developer's SDK patch.
    const assets = JSON.parse(readFileSync(resolve(root, 'native/screen-translate/obj/project.assets.json'), 'utf8'));
    const tessPackage = Object.keys(assets.packageFolders).map(folder=>resolve(folder,'tesseract/5.2.0')).find(existsSync);
    if (!tessPackage) throw new Error('Missing restored Tesseract package');
    mkdirSync(resolve(output,'x64'),{recursive:true});
    for(const file of ['tesseract50.dll','leptonica-1.82.0.dll']) copyFileSync(resolve(tessPackage,'x64',file),resolve(output,'x64',file));
    const packs = Object.values(assets.project.frameworks).flatMap(framework => framework.downloadDependencies ?? []);
    const notices = resolve(output, 'licenses');
    mkdirSync(notices, { recursive: true });
    const nativeNotices=resolve(root,'native/screen-translate/licenses');
    if(existsSync(nativeNotices)) for(const file of readdirSync(nativeNotices)) copyFileSync(resolve(nativeNotices,file),resolve(notices,file));
    for (const pack of packs.filter(pack => /^(Microsoft.NETCore.App.Runtime|Microsoft.WindowsDesktop.App.Runtime|Microsoft.Windows.SDK.NET.Ref)/i.test(pack.name))) {
        const version = pack.version.match(/^\[([^,\]]+)/)?.[1];
        if (!version) throw new Error(`Unrecognized runtime version: ${pack.version}`);
        const directory = Object.keys(assets.packageFolders).map(folder => resolve(folder, pack.name.toLowerCase(), version)).find(existsSync);
        if (!directory) throw new Error(`Missing restored runtime notices for ${pack.name}`);
        const files = readdirSync(directory).filter(file => /^(LICENSE|THIRD-PARTY-NOTICES)(\.|$)/i.test(file));
        // The Windows SDK targeting pack carries a license URL in its NuGet
        // metadata instead of a standalone license file; preserve that metadata.
        if (!files.length && pack.name === 'Microsoft.Windows.SDK.NET.Ref') {
            copyFileSync(resolve(directory, `${pack.name.toLowerCase()}.nuspec`), resolve(notices, `${pack.name}-${version}.nuspec`));
            continue;
        }
        if (!files.length) throw new Error(`Missing license in ${pack.name}`);
        for (const file of files) copyFileSync(resolve(directory, file), resolve(notices, `${pack.name}-${version}-${file}`));
    }
}

# Neeko Assistant Tauri Releases

Neeko Assistant usa el updater oficial de Tauri v2. El endpoint configurado en
`src-tauri/tauri.conf.json` espera un archivo `latest.json`, no `latest.yml`.

## 1. Generar la key de updater

Ejecuta una sola vez:

```powershell
npm run tauri signer generate -- -w "$env:USERPROFILE\.tauri\neeko-assistant.key"
```

El comando imprime una public key. Esa public key debe coincidir con:

```json
"plugins.updater.pubkey"
```

No subas la private key ni el password al repo.

## 2. Configurar GitHub Secrets

En GitHub: `Settings -> Secrets and variables -> Actions`.

Agrega:

```text
TAURI_SIGNING_PRIVATE_KEY
TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

`TAURI_SIGNING_PRIVATE_KEY` puede ser el contenido completo de la private key.
Si la key no tiene password, deja `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` vacio.

## 3. Publicar una version

Actualiza la version en estos tres archivos:

```text
package.json
src-tauri/Cargo.toml
src-tauri/tauri.conf.json
```

Luego crea y sube un tag:

```powershell
git tag v1.0.1
git push origin v1.0.1
```

El workflow `.github/workflows/release.yml` debe subir al GitHub Release:

```text
Neeko Assistant_*_x64-setup.exe
Neeko Assistant_*_x64-setup.exe.sig
latest.json
```

La app consulta:

```text
https://github.com/brian-dev25/neeko-assistant/releases/latest/download/latest.json
```

Si esa URL devuelve 404, la app no puede actualizar.

## 4. Build local firmado

Para generar artifacts localmente:

```powershell
npm run setup
```

Los archivos salen en:

```text
src-tauri/target/release/bundle/nsis/
```

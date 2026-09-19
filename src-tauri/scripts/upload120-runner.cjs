// Run the unmodified browser patcher locally, without a server or uploads.
const fs = require('node:fs');
const vm = require('node:vm');
const [enginePath, source, destination, optionsJson] = process.argv.slice(2);
global.window = {};
vm.runInThisContext(fs.readFileSync(enginePath, 'utf8'), { filename: enginePath });
const input = fs.readFileSync(source);
const info = window.Upload120Patcher.inspectMp4(input);
if (!info.isMp4 || info.error) throw new Error(info.error || 'El archivo no es MP4/MOV compatible.');
const options = JSON.parse(optionsJson);
const divider = options.divider === 'auto' ? (info.fps >= 100 ? 4 : info.fps >= 75 ? 3 : 2) : Number(options.divider);
const result = window.Upload120Patcher.patchMp4Buffer(input, { method: options.method, divider });
fs.writeFileSync(destination, result.bytes, { flag: 'wx' });
process.stdout.write(JSON.stringify({method: result.method, divider, warnings: result.warnings, fps: info.fps}));

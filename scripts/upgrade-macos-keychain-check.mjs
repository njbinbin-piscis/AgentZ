// Isolated adapter typecheck. This does not link or execute a macOS application.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID } from 'node:crypto';
import { spawnSync } from 'node:child_process';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const toolchain = process.argv[2] ?? 'stable';
const platform = process.argv[3] ?? 'macos';
if (!['macos', 'linux'].includes(platform)) throw new Error('Platform must be macos or linux');
const target = platform === 'macos' ? 'aarch64-apple-darwin' : 'x86_64-unknown-linux-gnu';
if (!/^[a-zA-Z0-9._-]+$/.test(toolchain)) throw new Error('Invalid toolchain name');
const output = path.join(root, '.cache', `${platform === 'macos' ? 'az077' : 'az078'}-adapter-check-${randomUUID()}`);
fs.mkdirSync(path.join(output, 'src'), { recursive: true });
fs.writeFileSync(path.join(output, 'Cargo.toml'), `[package]
name = "agentz-keychain-adapter-check"
version = "0.0.0"
edition = "2021"
[workspace]
[dependencies]
anyhow = "1"
uuid = { version = "1", features = ["v4"] }
chacha20poly1305 = "0.10"
getrandom = "0.2"
hex = "0.4"
tempfile = "3"
`);
if (platform === 'linux') {
    const dependency = JSON.stringify(path.join(root, 'vendor/secret-service').replaceAll('\\', '/'));
    fs.appendFileSync(path.join(output, 'Cargo.toml'), `secret-service = { path = ${dependency}, version = "=5.1.0", default-features = false, features = ["rt-async-io-crypto-rust", "agentz-no-prompts"] }\n`);
}
const source = name => JSON.stringify(path.join(root, 'vendor/piscis-engine/piscis-kernel/src/security', name).replaceAll('\\', '/'));
fs.writeFileSync(path.join(output, 'src/lib.rs'), `pub mod security {
#[path = ${source('secrets.rs')}]
pub mod secrets;
#[path = ${source(platform === 'macos' ? 'macos_keychain.rs' : 'linux_keyring.rs')}]
mod ${platform === 'macos' ? 'macos_keychain' : 'linux_keyring'};
}
`);
const args = [`+${toolchain}`, 'check', '--manifest-path', path.join(output, 'Cargo.toml'), '--target', target, '--tests', '--offline'];
const result = spawnSync('cargo', args, { cwd: root, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 });
fs.writeFileSync(path.join(output, 'check.log'), `${result.stdout ?? ''}${result.stderr ?? ''}${result.error?.message ?? ''}`);
const record = { toolchain, target, command: ['cargo', ...args], exit_code: result.status, scope: 'isolated adapter typecheck only; no native link or runtime acceptance', log: path.join(output, 'check.log') };
fs.writeFileSync(path.join(output, 'result.json'), JSON.stringify(record, null, 2));
console.log(JSON.stringify(record, null, 2));
process.exitCode = result.status ?? 1;

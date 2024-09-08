import { existsSync, readFileSync } from 'node:fs';
import fs from 'node:fs/promises';
import { type } from 'node:os';
import process from 'node:process';
import { consola } from 'consola';
import { execa } from 'execa';
import { join, resolve } from 'pathe';
import semver from 'semver';
import type { CheckedEnvironment } from '.';

const UPDATEKEY_LOCK_VERSION = '1';

export async function tauriUpdateKey(env: CheckedEnvironment): Promise<string | undefined> {
	if (process.env.TAURI_SIGNING_PRIVATE_KEY)
		return;

	const privateKeyPath = resolve(join(env.__deps, 'tauri.key'));
	const publicKeyPath = resolve(join(env.__deps, 'tauri.key.pub'));
	const updatekeyPath = resolve(join(env.__deps, 'updatekey_lock'));

	if (existsSync(updatekeyPath))
		if (readFileSync(updatekeyPath, 'utf-8') === UPDATEKEY_LOCK_VERSION)
			return;

	const readKeys = () => Promise.all([
		fs.readFile(publicKeyPath, 'utf-8'),
		fs.readFile(privateKeyPath, 'utf-8'),
	]);

	const keys = { privateKey: '', publicKey: '' };
	try {
		[keys.publicKey, keys.privateKey] = await readKeys();
		if (keys.privateKey === '' || keys.publicKey === '')
			consola.error(new Error(`empty keys`));
	}
	catch (error) {
		if (env.__debug) {
			consola.warn(`failed to read updater keys`);
			consola.error(error);
		}

		const quote = type() === 'Windows_NT' ? '"' : '\'';
		await execa`pnpm exec tauri signer generate --ci -w ${quote}${privateKeyPath}${quote}`;
		[keys.publicKey, keys.privateKey] = await readKeys();
		if (keys.privateKey === '' || keys.publicKey === '')
			throw new Error(`empty keys`);
	}

	process.env.TAURI_SIGNING_PRIVATE_KEY = keys.privateKey;
	process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD = '';
	await fs.writeFile(updatekeyPath, UPDATEKEY_LOCK_VERSION, 'utf-8');
	return keys.publicKey;
}

export async function patchTauri(env: CheckedEnvironment, targets: string[], args: string[]): Promise<string[]> {
	if (args.findIndex(a => ['--config', '-c'].includes(a)) !== -1)
		throw new Error('custom tauri build configuration is not supported!');

	const osType = type();
	const tauriPatch: {
		build: {
			features: string[];
		};
		bundle: {
			macOS: { minimumSystemVersion: string };
		};
		plugins: {
			updater: { pubkey?: string };
		};
	} = {
		build: {
			features: [],
		},
		bundle: {
			macOS: { minimumSystemVersion: '' },
		},
		plugins: {
			updater: {},
		},
	};

	const tauriRoot = join(env.__root, 'apps', 'desktop');
	const tauriConfig = JSON.parse(await fs.readFile(join(tauriRoot, 'tauri.conf.json'), 'utf-8'));

	switch (args[0]) {
		case 'dev':
			tauriPatch.build.features.push('devtools');
			break;
		case 'build':
			if (tauriConfig.bundle?.createUpdaterArtifacts !== false) {
				const pubKey = await tauriUpdateKey(env);
				if (pubKey != null)
					tauriPatch.plugins.updater.pubkey = pubKey;
			}
			break;
	}

	if (osType === 'Darwin') {
		const macOSStore = {
			defaultArm64: '11.0', // arm64 support was added in macOS 11.0 (but the safari feature support is bad)
			minimumVersion: tauriConfig?.bundle?.macOS?.minimumSystemVersion,
		};

		if (
			(targets.includes('aarch64-apple-darwin')
			|| (targets.length === 0 && process.arch === 'arm64'))
			&& (macOSStore.minimumVersion == null || semver.lt(
				semver.coerce(macOSStore.minimumVersion)!,
				semver.coerce(macOSStore.defaultArm64)!,
			))
		) {
			macOSStore.minimumVersion = macOSStore.defaultArm64;
			consola.log(`[aarch64-apple-darwin]: setting minimum system version to ${macOSStore.minimumVersion}`);
		}

		if (macOSStore.minimumVersion) {
			process.env.MACOSX_DEPLOYMENT_TARGET = macOSStore.minimumVersion;
			tauriPatch.bundle.macOS.minimumSystemVersion = macOSStore.minimumVersion;
		}
		else {
			throw new Error('no minimum macOS version found!');
		}
	}

	const tauriPatchC = join(tauriRoot, 'tauri.conf.patch.json');
	await fs.writeFile(tauriPatchC, JSON.stringify(tauriPatch, null, 2));

	args.splice(1, 0, '-c', tauriPatchC);
	return [tauriPatchC];
}

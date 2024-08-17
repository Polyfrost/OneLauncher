import process from 'node:process';
import fs from 'node:fs/promises';
import { type } from 'node:os';
import { join } from 'pathe';
import { execa } from 'execa';
import semver from 'semver';
import type { CheckedEnvironment } from '.';

export async function tauriUpdateKey(env: CheckedEnvironment): Promise<string | undefined> {
	if (process.env.TAURI_SIGNING_PRIVATE_KEY)
		return;

	const privateKeyPath = join(env.__root, '.keys', 'tauri.key');
	const publicKeyPath = join(env.__root, '.keys', 'tauri.key.pub');
	const readKeys = () => Promise.all([
		fs.readFile(publicKeyPath, 'utf-8'),
		fs.readFile(privateKeyPath, 'utf-8'),
	]);

	const keys = { privateKey: '', publicKey: '' };
	try {
		[keys.publicKey, keys.privateKey] = await readKeys();
		if (keys.privateKey === '' || keys.publicKey === '')
			throw new Error(`empty keys`);
	}
	catch (error) {
		if (env.__debug) {
			console.warn(`failed to read updater keys`);
			console.error(error);
		}

		const quote = type() === 'Windows_NT' ? '"' : '\'';
		await execa`pnpm desktop tauri signer generate --ci -w ${quote}${privateKeyPath}${quote}`;
		[keys.publicKey, keys.privateKey] = await readKeys();
		if (keys.privateKey === '' || keys.publicKey === '')
			throw new Error(`empty keys`);
	}

	process.env.TAURI_SIGNING_PRIVATE_KEY = keys.privateKey;
	process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD = '';
	return keys.publicKey;
}

export async function patchTauri(
	env: CheckedEnvironment,
	targets: string[],
	bundles: string[],
	args: string[],
): Promise<string[]> {
	if (args.findIndex(a => ['-c', '--config'].includes(a)) !== -1)
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
	if (bundles.length === 0) {
		const defaultBundles = tauriConfig?.bundle?.targets;
		if (Array.isArray(defaultBundles))
			bundles.push(...defaultBundles);
		if (bundles.length === 0)
			bundles.push('all');
	}

	switch (args[0]) {
		case 'dev':
			tauriPatch.build.features.push('devtools');
			break;
		case 'build':
			if (tauriConfig.plugins?.updater?.active) {
				const pubKey = await tauriUpdateKey(env);
				if (pubKey != null)
					tauriPatch.plugins.updater.pubkey = pubKey;
			}
			// tauriPatch.build.features.push('devtools');
			break;
	}

	if (osType === 'Darwin') {
		const macOSStore = {
			defaultArm64: '11.0',
			minimumVersion: tauriConfig?.bundle?.macOS?.minimumSystemVersion,
		};

		if (
			(targets.includes('aarch64-apple-darwin')
			|| (targets.length === 0 && process.arch === 'arm64'))
			&& (macOSStore.minimumVersion == null || semver.lt(
				semver.coerce(macOSStore.minimumVersion) as semver.SemVer,
				semver.coerce(macOSStore.defaultArm64) as semver.SemVer,
			))
		) {
			macOSStore.minimumVersion = macOSStore.defaultArm64;
			console.log(`aarch64-apple-darwin target: setting minimum system version to ${macOSStore.minimumVersion}`);
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

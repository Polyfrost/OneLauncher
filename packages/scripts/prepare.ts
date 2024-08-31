import fs from 'node:fs';
import { readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'pathe';
import mustache from 'mustache';
import consola from 'consola';
import { checkEnvironment, which } from './utils';
import { getTriple } from './utils/triple';

const env = checkEnvironment(import.meta);
const triple = getTriple();
const PREPARE_LOCK_VERSION = '1';
const prepareLockPath = resolve(join(env.__deps, 'prepare_lock'));

if ((await Promise.all([which`cargo`, which`rustc`, which`pnpm`])).some(f => !f))
	consola.error(
		`
		Basic OneLauncher dependencies missing!
		Ensure you have rust and pnpm installed:
		https://rustup.rs
		https://pnpm.io

		And that you have run the setup script:
		packages/scripts/${triple[0] === 'Windows_NT' ? 'setup.ps1' : 'setup.sh'}
		`,
	);

consola.info('generating cargo configuration file.');

if (fs.existsSync(prepareLockPath))
	if (fs.readFileSync(prepareLockPath, 'utf-8') === PREPARE_LOCK_VERSION)
		env.__exit(0);

interface ConfigStore {
	isWin: boolean;
	isMacOS: boolean;
	isLinux: boolean;
	hasLLD: boolean | { linker: string };
}

const configStore: ConfigStore = {
	isWin: false,
	isMacOS: false,
	isLinux: false,
	hasLLD: false,
};

try {
	switch (triple[0]) {
		case 'Linux':
			configStore.isLinux = true;
			if (await which`clang`)
				if (await which`mold`)
					configStore.hasLLD = { linker: 'mold' };
				else if (await which`lld`)
					configStore.hasLLD = { linker: 'lld' };
			break;
		case 'Darwin':
			configStore.isMacOS = true;
			break;
		case 'Windows_NT':
			configStore.isWin = true;
			configStore.hasLLD = await which`lld-link`;
			break;
	}

	const template = await readFile(join(env.__root, '.cargo', 'config.toml.mustache'), { encoding: 'utf8' });
	const rendered = mustache.render(template, configStore).replace(/\n{2,}/g, '\n');
	await writeFile(join(env.__root, '.cargo', 'config.toml'), rendered, { mode: 0o751, flag: 'w+', encoding: 'utf-8' });
	await writeFile(join(env.__deps, 'prepare_lock'), PREPARE_LOCK_VERSION, 'utf-8');
}
catch (error) {
	consola.error(`
		failed to generate .config/cargo.toml.
		this is probably a bug, please open an issue with system info at
		https://github.com/polyfrost/onelauncher/issues/new/choose
	`);
	if (env.__debug)
		consola.error(error);
	env.__exit(1);
}

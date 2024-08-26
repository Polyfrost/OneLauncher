import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'pathe';
import mustache from 'mustache';
import { checkEnvironment, which } from './utils';
import { getTriple } from './utils/triple';

const { __debug, __exit, __root } = checkEnvironment(import.meta);
const triple = getTriple();

if ((await Promise.all([which`cargo`, which`rustc`, which`pnpm`])).some(f => !f))
	console.error(
		`
		Basic OneLauncher dependencies missing!
		Ensure you have rust and pnpm installed:
		https://rustup.rs
		https://pnpm.io

		And that you have run the setup script:
		packages/scripts/${triple[0] === 'Windows_NT' ? 'setup.ps1' : 'setup.sh'}
		`,
	);

console.log('generating cargo configuration file.');

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

	const template = await readFile(join(__root, '.cargo', 'config.toml.mustache'), { encoding: 'utf8' });
	const rendered = mustache.render(template, configStore).replace(/\n{2,}/g, '\n');
	await writeFile(join(__root, '.cargo', 'config.toml'), rendered, { mode: 0o751, flag: 'w+' });
}
catch (error) {
	console.error(`
		failed to generate .config/cargo.toml.
		this is probably a bug, please open an issue with system info at
		https://github.com/polyfrost/onelauncher/issues/new/choose
	`);
	if (__debug)
		console.error(error);
	__exit(1);
}

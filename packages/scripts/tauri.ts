import process from 'node:process';
import fs from 'node:fs/promises';
import { setTimeout } from 'node:timers/promises';
import pathe from 'pathe';
import { execa } from 'execa';
import { awaitLock, checkEnvironment } from './utils';
import { patchTauri } from './utils/patch';

const env = checkEnvironment(import.meta);
const { __dirname, __root, __exit } = env;
const [_, __, ...args] = process.argv;
const __distribution = pathe.join(__root, 'packages', 'distribution');
const __desktop = pathe.join(__root, 'apps', 'desktop');
const __cleanup: string[] = [];

const cleanup = () => Promise.all(__cleanup.map(f => fs.unlink(f).catch((_) => {})));
const exists = (path: string) => fs.access(path, fs.constants.R_OK).then(_ => true).catch(_ => false);

process.on('SIGINT', cleanup);
if (args.length === 0)
	args.push('build');

function targetFilter(filter: 'b' | 't') {
	const filters = filter === 'b' ? ['-b', '--bundles'] : ['-t', '--target'];
	return (_: string, idx: number, args: string[]) => {
		if (idx === 0)
			return false;
		const prev = args[idx - 1] ?? '';
		return filters.includes(prev);
	};
}

const targets = args.filter(targetFilter('t')).flatMap(t => t.split(','));
const bundles = args.filter(targetFilter('b')).flatMap(t => t.split(','));

const store = { code: 0 };

try {
	switch (args[0]) {
		case 'dev': {
			__cleanup.push(...(await patchTauri(env, targets, bundles, args)));
			switch (process.platform) {
				case 'linux':
				case 'darwin':
					void awaitLock(pathe.join(__root, 'target', 'debug', '.cargo-lock'))
						.then(_ => setTimeout(1000).then(cleanup), (_) => {});
					break;
			}

			break;
		}
		case 'build': {
			if (!process.env.NODE_OPTIONS || !process.env.NODE_OPTIONS.includes('--max_old_space_size'))
				process.env.NODE_OPTIONS = `--max_old_space_size=4096 ${process.env.NODE_OPTIONS ?? ''}`;

			process.env.GENERATE_SOURCEMAP = 'false';
			__cleanup.push(...(await patchTauri(env, targets, bundles, args)));

			if (process.platform === 'darwin') {
				process.env.BACKGROUND_FILE = pathe.resolve(__distribution, 'macos', 'dmg.png');
				process.env.BACKGROUND_FILE_NAME = pathe.basename(process.env.BACKGROUND_FILE);
				process.env.BACKGROUND_CLAUSE = `set background picture of opts to file ".background:${process.env.BACKGROUND_FILE_NAME}"`;

				if (!(await exists(process.env.BACKGROUND_FILE)))
					console.warn(`dmg background file not found at ${process.env.BACKGROUND_FILE}`);

				break;
			}
		}
	}

	await execa('pnpm', ['desktop', 'tauri', ...args], { cwd: __desktop });

	if (args[0] === 'build' && bundles.some(b => b === 'deb' || b === 'all')) {
		const linuxTargets = targets.filter(t => t.includes('-linux-'));
		if (linuxTargets.length > 0)
			linuxTargets.forEach(async (t) => {
				process.env.TARGET = t;
				await execa(pathe.join(__dirname, 'fix-deb.sh'), [], { cwd: __dirname });
			});
		else if (process.platform === 'linux')
			await execa(pathe.join(__dirname, 'fix-deb.sh'), [], { cwd: __dirname });
	}
}
catch (error: any) {
	console.error(`tauri ${args[0]} failed with exit code ${typeof error === 'number' ? error : 1}`);
	console.warn(`to fix some errors, run ${process.platform === 'win32' ? './packages/scripts/setup.ps1' : './packages/scripts/setup.sh'}`);

	if (typeof error === 'number') {
		store.code = error;
	}
	else {
		if (error instanceof Error)
			console.error(error);
		store.code = 1;
	}
}
finally {
	cleanup();
	__exit(store.code);
}

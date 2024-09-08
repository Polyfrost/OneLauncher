import fs from 'node:fs/promises';
import process from 'node:process';
import { setTimeout } from 'node:timers/promises';
import { consola } from 'consola';
import { execa } from 'execa';
import pathe from 'pathe';
import { awaitLock, checkEnvironment } from './utils';
import { patchTauri } from './utils/patch';

const env = checkEnvironment(import.meta);
const [_, __, ...args] = process.argv;
const __desktop = pathe.join(env.__root, 'apps', 'desktop');
const store = { code: 0, cleanup: new Array<string>() };
const cleanup = () => Promise.all(store.cleanup.map(f => fs.unlink(f).catch((_) => {})));

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
const bundles = args.filter(targetFilter('b')).flatMap(b => b.split(','));

try {
	switch (args[0]) {
		case 'dev': {
			store.cleanup.push(...(await patchTauri(env, targets, args)));
			switch (process.platform) {
				case 'linux':
				case 'darwin':
					void awaitLock(pathe.join(env.__root, 'target', 'debug', '.cargo-lock'))
						.then(_ => setTimeout(1000).then(cleanup), (_) => {});
					break;
			}

			break;
		}
		case 'build': {
			if (!process.env.NODE_OPTIONS || !process.env.NODE_OPTIONS.includes('--max_old_space_size'))
				process.env.NODE_OPTIONS = `--max_old_space_size=4096 ${process.env.NODE_OPTIONS ?? ''}`;

			process.env.GENERATE_SOURCEMAP = 'false';
			store.cleanup.push(...(await patchTauri(env, targets, args)));
		}
	}

	await execa('pnpm', ['exec', 'tauri', ...args], { cwd: __desktop });

	if (args[0] === 'build' && bundles.some(b => b === 'deb' || b === 'all')) {
		const linuxTargets = targets.filter(t => t.includes('-linux-'));
		if (linuxTargets.length > 0)
			linuxTargets.forEach(async (t) => {
				process.env.TARGET = t;
				await execa(pathe.join(env.__dirname, 'fix-deb.sh'), [], { cwd: env.__dirname });
			});
		else if (process.platform === 'linux')
			await execa(pathe.join(env.__dirname, 'fix-deb.sh'), [], { cwd: env.__dirname });
	}
}
catch (error: any) {
	consola.error(`tauri ${args[0]} failed with exit code ${typeof error === 'number' ? error : 1}`);
	consola.warn(`to fix some errors, run ${process.platform === 'win32' ? './packages/scripts/setup.ps1' : './packages/scripts/setup.sh'}`);

	if (typeof error === 'number') {
		store.code = error;
	}
	else {
		if (error instanceof Error)
			consola.error(error);
		store.code = 1;
	}
}
finally {
	cleanup();
	env.__exit(store.code);
}

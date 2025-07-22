import fs from 'node:fs/promises';
import { type } from 'node:os';
import process from 'node:process';

import { setTimeout } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { consola } from 'consola';
import { execa } from 'execa';
import { dirname, join, resolve } from 'pathe';

export type CheckedEnvironment = ReturnType<typeof checkEnvironment>;

export function checkEnvironment(meta: ImportMeta) {
	if (/^(?:msys|mingw|cygwin)$/i.test(process.env.OSTYPE ?? '')) {
		consola.error('bash for windows is not supported. please use Powershell or cmd');
		process.exit(255);
	}

	process.umask(0o026);

	const __filename = fileURLToPath(meta.url);
	const __dirname = dirname(__filename);
	const __debug = process.env.NODE_ENV === 'debug';
	const __root = resolve(join(__dirname, '..', '..'));
	const __exit = process.exit;

	return { __filename, __dirname, __debug, __root, __exit };
}

async function where(cmd: string): Promise<boolean> {
	if (/\\/.test(cmd))
		return false;

	try {
		await execa`where "${cmd}"`;
	}
	catch {
		return false;
	}

	return true;
}

export async function which(cmd: TemplateStringsArray): Promise<boolean> {
	return type() === 'Windows_NT'
		? where(cmd[0]!)
		: Promise.any(
				Array.from(new Set(process.env.PATH?.split(':')))
					.map(p => fs.access(join(p, cmd[0]!), fs.constants.X_OK)),
			).then(_ => true, _ => false);
}

export async function awaitLock(file: string): Promise<void> {
	if (!(await which`flock`))
		throw new Error('flock is not installed!');

	consola.start(`waiting for file lock on ${file}`);
	const store = { locked: false };
	while (!store.locked)
		try {
			await execa`flock -ns "${file}" -c true`;
			await setTimeout(100);
		}
		catch {
			store.locked = true;
		}

	while (store.locked) {
		try {
			await execa`flock -ns "${file}" -c true`;
		}
		catch {
			await setTimeout(100);
			continue;
		}

		store.locked = false;
	}
}

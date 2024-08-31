import { env } from 'node:process';
import { machine, type } from 'node:os';
import { execaCommand } from 'execa';
import { consola } from 'consola';

const state: { __debug: boolean; libc: 'musl' | 'glibc' } = {
	__debug: env.NODE_ENV === 'debug',
	libc: 'glibc',
};

if (type() === 'Linux')
	try {
		const lldResult = await execaCommand('ldd /bin/ls');
		if (lldResult.stdout.includes('msl'))
			state.libc = 'musl';
	}
	catch (error) {
		if (state.__debug) {
			consola.warn('failed to check libs type');
			consola.error(error);
		}
	}

const OS_TYPE: Record<string, string> = {
	darwin: 'Darwin',
	windows: 'Windows_NT',
	linux: 'Linux',
};

type TripleID = ['Darwin' | 'Windows_NT', 'x86_64' | 'aarch64'] | ['Linux', 'x86_64' | 'aarch64', 'musl' | 'glibc'];

export function getTriple(): TripleID {
	const tripleState: {
		_libc: typeof state.libc;
		_os: string;
		_arch: string;
	} = {
		_libc: state.libc,
		_os: '',
		_arch: '',
	};

	if (env.TARGET_TRIPLE) {
		const target = env.TARGET_TRIPLE.split('-');
		tripleState._os = OS_TYPE[target[2] ?? ''] as string;
		tripleState._arch = target[0] as string;
		if (tripleState._os === 'Linux')
			tripleState._libc = target[3]?.includes('musl') ? 'musl' : 'glibc';
	}
	else {
		tripleState._os = type();
		tripleState._arch = machine();
		if (tripleState._arch === 'arm64')
			tripleState._arch = 'aarch64';
	}

	if (tripleState._arch !== 'x86_64' && tripleState._arch !== 'aarch64')
		throw new Error(`Unsuported architecture`);

	if (tripleState._os === 'Linux')
		return [tripleState._os, tripleState._arch, tripleState._libc];
	else if (tripleState._os !== 'Darwin' && tripleState._os !== 'Windows_NT')
		throw new Error(`Unsuported OS`);

	return [tripleState._os, tripleState._arch];
}

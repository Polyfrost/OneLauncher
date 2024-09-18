import os from 'node:os';
import process from 'node:process';
import { consola } from 'consola';
import { execa } from 'execa';

const state: { debug: boolean; libc: 'musl' | 'glibc' } = {
	debug: process.env.NODE_ENV === 'debug',
	libc: 'glibc',
};

if (os.type() === 'Linux')
	try {
		if ((await execa`ldd /bin/ls`).stdout.includes('musl'))
			state.libc = 'musl';
	}
	catch (error) {
		if (state.debug) {
			consola.warn('failed to check libs type');
			consola.error(error);
		}
	}

const OS_TYPE: Record<string, string> = {
	darwin: 'Darwin',
	windows: 'Windows_NT',
	linux: 'Linux',
};

type TripleID =
	| ['Darwin' | 'Windows_NT', 'x86_64' | 'aarch64']
	| ['Linux', 'x86_64' | 'aarch64', 'musl' | 'glibc'];

export function getTriple(): TripleID {
	const tripleState: {
		libc: typeof state.libc;
		os: string;
		arch: string;
	} = {
		libc: state.libc,
		os: '',
		arch: '',
	};

	if (process.env.TARGET_TRIPLE) {
		const target = process.env.TARGET_TRIPLE.split('-');
		tripleState.os = OS_TYPE[target[2] ?? ''] as string;
		tripleState.arch = target[0] as string;
		if (tripleState.os === 'Linux')
			tripleState.libc = target[3]?.includes('musl') ? 'musl' : 'glibc';
	}
	else {
		tripleState.os = os.type();
		tripleState.arch = os.machine();
		if (tripleState.arch === 'arm64')
			tripleState.arch = 'aarch64';
	}

	if (tripleState.arch !== 'x86_64' && tripleState.arch !== 'aarch64')
		throw new Error(`Unsuported architecture`);

	if (tripleState.os === 'Linux')
		return [tripleState.os, tripleState.arch, tripleState.libc];
	else if (tripleState.os !== 'Darwin' && tripleState.os !== 'Windows_NT')
		throw new Error(`Unsuported OS`);

	return [tripleState.os, tripleState.arch];
}

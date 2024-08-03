#!/usr/bin/env node

import { checkEnvironment, which } from './utils';
import { getTriple } from './utils/triple';

checkEnvironment(import.meta);
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

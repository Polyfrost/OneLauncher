import process from 'node:process';
import client from '@actions/artifact';
import core from '@actions/core';
import glob from '@actions/glob';
import io from '@actions/io';
import { exists } from '@actions/io/lib/io-util';

type OS = 'darwin' | 'windows' | 'linux';
type Arch = 'x64' | 'arm64';
type Profile = 'debug' | 'release' | 'dev' | 'dev-debug';

interface Updater {
	bundle: string;
	bundleExt: string;
	archiveExt: string;
}
interface TargetConfig {
	bundle: string;
	ext: string;
}
interface BuildTarget {
	updater: false | Updater;
	standalone: TargetConfig[];
}

const osTargets = {
	darwin: {
		updater: {
			bundle: 'macos',
			bundleExt: 'app',
			archiveExt: 'tar.gz',
		},
		standalone: [
			{ ext: 'dmg', bundle: 'dmg' },
		],
	},
	windows: {
		updater: {
			bundle: 'msi',
			bundleExt: 'msi',
			archiveExt: 'zip',
		},
		standalone: [
			{ ext: 'msi', bundle: 'msi' },
		],
	},
	linux: {
		updater: false, // TODO
		standalone: [
			{ ext: 'deb', bundle: 'deb' },
		],
	},
} satisfies Record<OS, BuildTarget>;

const getInput = <T = string>(name: string): T => core.getInput(name) as T;
const os: OS = getInput('os');
const arch: Arch = getInput('arch');
const profile: Profile = getInput('profile');
const target = getInput('target');
const binaryName = getInput<string | undefined>('binary');

const targetDir = `target/${target}`;
const bundleDir = `${targetDir}/${profile}/bundle`;
const artifactsDir = '.artifacts';
const artifactBase = `OneLauncher-${os}-${arch}`;
const frontendBundle = 'apps/frontend/dist.tar.xz';
const updaterName = `OneLauncher-Updater-${os}-${arch}`;
const frontendName = `OneLauncher-Frontend-${os}-${arch}`;

const globFiles = async (pattern: string): Promise<string[]> => await (await glob.create(pattern)).glob();

async function uploadFrontend() {
	if (!(await exists(frontendBundle))) {
		console.error('frontend archive missing!');
		return;
	}

	const artifactName = `${frontendName}.tar.xz`;
	const artifactPath = `${artifactsDir}/${artifactName}`;

	await io.cp(frontendBundle, artifactPath);
	await client.uploadArtifact(artifactName, [artifactPath], artifactsDir);
}

async function uploadUpdater(updater: BuildTarget['updater']) {
	if (!updater)
		return;

	const { bundle, bundleExt, archiveExt } = updater;
	const fullExt = `${bundleExt}.${archiveExt}`;
	const files = await globFiles(`${bundleDir}/${bundle}/*.${fullExt}*`);
	const updaterPath = files.find(f => f.endsWith(fullExt));

	if (!updaterPath)
		throw new Error(`failed to find updater path in ${files.join(',')}`);

	const artifactPath = `${artifactsDir}/${updaterName}.${archiveExt}`;
	await io.cp(updaterPath, artifactPath);
	await io.cp(`${updaterPath}.sig`, `${artifactPath}.sig`);
	await client.uploadArtifact(updaterName, [artifactPath, `${artifactPath}.sig`], artifactsDir);
}

async function uploadStandalone({ bundle, ext }: TargetConfig) {
	const files = await globFiles(`${bundleDir}/${bundle}/*.${ext}*`);
	const standalonePath = files.find(f => f.endsWith(ext));

	if (!standalonePath)
		throw new Error(`failed to find standalone path in ${files.join(',')}`);

	const artifactName = `${artifactBase}.${ext}`;
	const artifactPath = `${artifactsDir}/${artifactName}`;
	await io.cp(standalonePath, artifactPath, { recursive: true });
	await client.uploadArtifact(artifactName, [artifactPath], artifactsDir);
}

async function uploadBinary() {
	if (typeof binaryName !== 'string')
		return;

	const file = binaryName || 'onelauncher_gui';
	const binaryPath = `${targetDir}/${file}`;

	if (!(await exists(binaryPath))) {
		console.error('binary missing!');
		return;
	}

	await io.cp(binaryPath, `${artifactsDir}/${file}`);
	await client.uploadArtifact(file, [`${artifactsDir}/${file}`], artifactsDir);
}

async function run() {
	await io.mkdirP(artifactsDir);
	const { updater, standalone } = osTargets[os];

	await Promise.all([
		uploadUpdater(updater),
		uploadFrontend(),
		uploadBinary(),
		...standalone.map(uploadStandalone),
	]);
}

run().catch((error) => {
	console.error(error);
	process.exit(1);
});

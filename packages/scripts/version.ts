import type { ReleaseType } from 'semver';
import { readFile, writeFile } from 'node:fs/promises';
import process from 'node:process';
import { consola } from 'consola';
import { join } from 'pathe';
import semver from 'semver';
import { parse as parseTOML } from 'smol-toml';
import { checkEnvironment } from './utils';

const env = checkEnvironment(import.meta);

const RELEASE_TYPES: Set<ReleaseType> = new Set([
	'major',
	'minor',
	'patch',
	'premajor',
	'preminor',
	'prepatch',
	'prerelease',
]);

const VERSIONED_PACKAGE_JSON_PATHS = [
	'package.json',
	'packages/core/package.json',
	'packages/gamemode/package.json',
	'packages/scripts/package.json',
	'apps/oneclient/desktop/package.json',
	'apps/onelauncher/desktop/package.json',
	'.github/actions/publish-artifacts/package.json',
] as const;

interface PackageJsonLike {
	version?: unknown;
	[key: string]: unknown;
}

interface CargoWorkspaceManifest {
	workspace?: {
		members?: unknown;
	};
	package?: {
		name?: unknown;
		version?: unknown;
	};
}

function usage() {
	consola.info(`
usage:
  pnpm version:bump <version>
  pnpm version:bump <major|minor|patch|premajor|preminor|prepatch|prerelease> [preid]
  pnpm version:bump <...> --dry-run
`);
}

function inferPreid(version: string): string {
	const prerelease = semver.prerelease(version);
	const preid = prerelease?.find(part => typeof part === 'string');
	return typeof preid === 'string' ? preid : 'alpha';
}

function hasWorkspaceVersion(value: unknown): boolean {
	return typeof value === 'object'
		&& value !== null
		&& 'workspace' in value
		&& (value as { workspace?: unknown }).workspace === true;
}

function resolveNextVersion(currentVersion: string, positionalArgs: Array<string>): string | null {
	const [firstArg, secondArg] = positionalArgs;
	if (!firstArg)
		return null;

	if (RELEASE_TYPES.has(firstArg as ReleaseType)) {
		const releaseType = firstArg as ReleaseType;
		const usePreid = releaseType.startsWith('pre') || releaseType === 'prerelease';
		if (!usePreid && secondArg)
			return null;
		return semver.inc(currentVersion, releaseType, usePreid ? (secondArg ?? inferPreid(currentVersion)) : undefined);
	}

	if (secondArg)
		return null;

	return semver.valid(firstArg);
}

async function readRootVersion(): Promise<string> {
	const raw = await readFile(join(env.__root, 'package.json'), 'utf8');
	const parsed = JSON.parse(raw) as PackageJsonLike;
	if (typeof parsed.version !== 'string')
		throw new Error('root package.json does not contain a valid version string');

	return parsed.version;
}

async function updatePackageJson(path: string, version: string, dryRun: boolean): Promise<boolean> {
	const fullPath = join(env.__root, path);
	const raw = await readFile(fullPath, 'utf8');
	const parsed = JSON.parse(raw) as PackageJsonLike;

	if (typeof parsed.version !== 'string')
		throw new Error(`${path} does not contain a valid version string`);

	if (parsed.version === version)
		return false;

	parsed.version = version;
	if (!dryRun)
		await writeFile(fullPath, `${JSON.stringify(parsed, null, '\t')}\n`, 'utf8');

	return true;
}

async function getWorkspacePackageNames(): Promise<Set<string>> {
	const workspaceManifestPath = join(env.__root, 'Cargo.toml');
	const workspaceRaw = await readFile(workspaceManifestPath, 'utf8');
	const workspaceManifest = parseTOML(workspaceRaw) as CargoWorkspaceManifest;
	const members = workspaceManifest.workspace?.members;

	if (!Array.isArray(members))
		throw new Error('failed to parse workspace members from Cargo.toml');

	const names: Set<string> = new Set();

	await Promise.all(members.map(async (member) => {
		if (typeof member !== 'string')
			return;

		const memberManifestPath = join(env.__root, member, 'Cargo.toml');
		const memberRaw = await readFile(memberManifestPath, 'utf8');
		const memberManifest = parseTOML(memberRaw) as CargoWorkspaceManifest;

		const crateName = memberManifest.package?.name;
		const crateVersion = memberManifest.package?.version;
		if (typeof crateName === 'string' && hasWorkspaceVersion(crateVersion))
			names.add(crateName);
	}));

	return names;
}

async function updateCargoWorkspaceVersion(version: string, dryRun: boolean): Promise<boolean> {
	const cargoPath = join(env.__root, 'Cargo.toml');
	const cargoRaw = await readFile(cargoPath, 'utf8');
	const workspaceVersionPattern = /(\[workspace\.package\][\s\S]*?\nversion\s*=\s*")([^"]+)(")/;
	if (!workspaceVersionPattern.test(cargoRaw))
		throw new Error('failed to find [workspace.package].version in Cargo.toml');

	const nextCargo = cargoRaw.replace(workspaceVersionPattern, `$1${version}$3`);
	if (nextCargo === cargoRaw)
		return false;

	if (!dryRun)
		await writeFile(cargoPath, nextCargo, 'utf8');

	return true;
}

async function updateCargoLock(version: string, crateNames: Set<string>, dryRun: boolean): Promise<boolean> {
	const cargoLockPath = join(env.__root, 'Cargo.lock');
	const cargoLockRaw = await readFile(cargoLockPath, 'utf8');
	const packagePattern = /(\[\[package\]\]\r?\nname = "([^"]+)"\r?\nversion = ")([^"]+)(")/g;

	const nextCargoLock = cargoLockRaw.replace(packagePattern, (match, prefix, packageName, currentVersion, suffix) => {
		if (!crateNames.has(packageName))
			return match;

		if (currentVersion === version)
			return match;

		return `${prefix}${version}${suffix}`;
	});

	if (nextCargoLock === cargoLockRaw)
		return false;

	if (!dryRun)
		await writeFile(cargoLockPath, nextCargoLock, 'utf8');

	return true;
}

async function main() {
	const rawArgs = process.argv.slice(2);
	const dryRun = rawArgs.includes('--dry-run');
	const positionalArgs = rawArgs.filter(arg => arg !== '--dry-run');

	if (positionalArgs.length === 0 || positionalArgs.length > 2) {
		usage();
		throw new Error('invalid arguments');
	}

	const currentVersion = await readRootVersion();
	const nextVersion = resolveNextVersion(currentVersion, positionalArgs);
	if (!nextVersion) {
		usage();
		throw new Error('unable to resolve target version');
	}

	if (currentVersion === nextVersion) {
		consola.warn(`version already set to ${nextVersion}`);
		return;
	}

	consola.start(`${dryRun ? 'previewing' : 'bumping'} version: ${currentVersion} -> ${nextVersion}`);
	const changedPaths: Array<string> = [];

	for (const packagePath of VERSIONED_PACKAGE_JSON_PATHS)
		if (await updatePackageJson(packagePath, nextVersion, dryRun))
			changedPaths.push(packagePath);

	if (await updateCargoWorkspaceVersion(nextVersion, dryRun))
		changedPaths.push('Cargo.toml');

	const crateNames = await getWorkspacePackageNames();
	if (await updateCargoLock(nextVersion, crateNames, dryRun))
		changedPaths.push('Cargo.lock');

	if (changedPaths.length === 0) {
		consola.warn('no files required changes');
		return;
	}

	const action = dryRun ? 'would update' : 'updated';
	consola.success(`${action} ${changedPaths.length} files:`);
	changedPaths.forEach((path) => {
		consola.info(`- ${path}`);
	});
}

void main().catch((error: unknown) => {
	consola.error(error instanceof Error ? error.message : String(error));
	env.__exit(1);
});

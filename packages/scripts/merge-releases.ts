import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import process from 'node:process';
import { consola } from 'consola';
import { execa } from 'execa';
import { join } from 'pathe';
import { checkEnvironment } from './utils';

const env = checkEnvironment(import.meta);

interface ReleaseAsset {
	id: string;
	name: string;
	label: string;
	apiUrl: string;
	size: number;
}

interface Release {
	tagName: string;
	name: string;
	isDraft: boolean;
	assets: Array<ReleaseAsset>;
	body: string;
}

interface LatestJson {
	version: string;
	notes: string;
	pub_date: string;
	platforms: Record<string, { signature: string; url: string }>;
}

async function ghJson<T>(...args: Array<string>): Promise<T> {
	const { stdout } = await execa('gh', args);
	return JSON.parse(stdout) as T;
}

async function gh(...args: Array<string>): Promise<void> {
	await execa('gh', args, { stdio: 'inherit' });
}

async function getRepo(): Promise<string> {
	const { nameWithOwner } = await ghJson<{ nameWithOwner: string }>('repo', 'view', '--json', 'nameWithOwner');
	return nameWithOwner;
}

async function main() {
	consola.start('fetching draft releases...');

	const allReleases = await ghJson<Array<{ tagName: string; isDraft: boolean; name: string }>>(
		'release',
		'list',
		'--json',
		'tagName,isDraft,name',
		'--limit',
		'20',
	);

	const draftTags = allReleases.filter(r => r.isDraft).map(r => r.tagName);

	if (draftTags.length < 2) {
		consola.warn(`only ${draftTags.length} draft release(s) found, nothing to merge`);
		return;
	}

	consola.info(`found ${draftTags.length} draft releases, fetching details...`);

	const drafts = await Promise.all(draftTags.map(tag => ghJson<Release>('release', 'view', tag, '--json', 'tagName,name,isDraft,assets,body')));

	for (const draft of drafts)
		consola.info(`  - ${draft.name} (${draft.tagName}): ${draft.assets.length} assets`);

	const [target, ...sources] = drafts as [Release, ...Array<Release>];
	const repo = await getRepo();

	consola.info(`\ntarget: ${target.name} (${target.tagName})`);
	for (const source of sources)
		consola.info(`source: ${source.name} (${source.tagName})`);

	const tmpDir = await mkdtemp(join(tmpdir(), 'merge-releases-'));

	try {
		// Download and parse latest.json from target
		let mergedLatestJson: LatestJson | null = null;
		const targetLatestAsset = target.assets.find(a => a.name === 'latest.json');

		if (targetLatestAsset) {
			const path = join(tmpDir, 'latest-target.json');
			await gh('release', 'download', target.tagName, '--pattern', 'latest.json', '--output', path);
			mergedLatestJson = JSON.parse(await readFile(path, 'utf8')) as LatestJson;
			consola.success(`loaded latest.json from target (${Object.keys(mergedLatestJson.platforms).length} platforms)`);
		}

		for (const source of sources) {
			consola.start(`merging ${source.tagName} into ${target.tagName}...`);

			// Merge latest.json
			const sourceLatestAsset = source.assets.find(a => a.name === 'latest.json');

			if (sourceLatestAsset) {
				const path = join(tmpDir, 'latest-source.json');
				await gh('release', 'download', source.tagName, '--pattern', 'latest.json', '--output', path);
				const sourceLatestJson = JSON.parse(await readFile(path, 'utf8')) as LatestJson;

				if (mergedLatestJson) {
					const before = Object.keys(mergedLatestJson.platforms).length;
					mergedLatestJson.platforms = { ...mergedLatestJson.platforms, ...sourceLatestJson.platforms };
					const added = Object.keys(mergedLatestJson.platforms).length - before;

					if (new Date(sourceLatestJson.pub_date) > new Date(mergedLatestJson.pub_date))
						mergedLatestJson.pub_date = sourceLatestJson.pub_date;

					if (!mergedLatestJson.notes && sourceLatestJson.notes)
						mergedLatestJson.notes = sourceLatestJson.notes;

					consola.success(`merged latest.json: added ${added} platforms (total: ${Object.keys(mergedLatestJson.platforms).length})`);
				}
				else {
					mergedLatestJson = sourceLatestJson;
					consola.success(`loaded latest.json from source (${Object.keys(mergedLatestJson.platforms).length} platforms)`);
				}
			}

			// Download and re-upload all non-latest.json assets
			const assetsToMove = source.assets.filter(a => a.name !== 'latest.json');

			if (assetsToMove.length > 0) {
				consola.start(`downloading ${assetsToMove.length} assets from ${source.tagName}...`);

				for (const asset of assetsToMove) {
					const assetPath = join(tmpDir, asset.name);
					await gh('release', 'download', source.tagName, '--pattern', asset.name, '--output', assetPath);
					consola.info(`  downloaded ${asset.name} (${(asset.size / 1024 / 1024).toFixed(1)} MB)`);
				}

				consola.start(`uploading ${assetsToMove.length} assets to ${target.tagName}...`);
				const assetPaths = assetsToMove.map(a => join(tmpDir, a.name));
				await gh('release', 'upload', target.tagName, '--clobber', ...assetPaths);
				consola.success(`uploaded ${assetsToMove.length} assets to ${target.tagName}`);
			}

			// Delete source release
			consola.start(`deleting source release ${source.tagName}...`);
			await gh('release', 'delete', source.tagName, '--yes');
			consola.success(`deleted ${source.tagName}`);
		}

		// Replace latest.json on target with merged version
		if (mergedLatestJson) {
			if (targetLatestAsset) {
				const numericId = targetLatestAsset.apiUrl.split('/').pop()!;
				consola.start(`deleting old latest.json (asset ${numericId}) from target...`);
				await gh('api', '-X', 'DELETE', `/repos/${repo}/releases/assets/${numericId}`);
			}

			const mergedPath = join(tmpDir, 'latest.json');
			await writeFile(mergedPath, `${JSON.stringify(mergedLatestJson, null, 2)}\n`, 'utf8');

			consola.start('uploading merged latest.json...');
			await gh('release', 'upload', target.tagName, mergedPath);

			consola.success(`merged latest.json platforms: ${Object.keys(mergedLatestJson.platforms).join(', ')}`);
		}

		consola.success(`\ndone! all releases merged into ${target.tagName}`);
	}
	finally {
		await rm(tmpDir, { recursive: true, force: true });
	}
}

void main().catch((error: unknown) => {
	consola.error(error instanceof Error ? error.message : String(error));
	env.__exit(1);
});

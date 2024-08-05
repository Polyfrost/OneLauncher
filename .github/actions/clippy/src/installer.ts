import { arch as getArch, platform as getPlatform } from 'node:process';
import { join } from 'pathe';
import { Headers, HttpClient, HttpCodes } from '@actions/http-client';
import { downloadTool, extractTar } from '@actions/tool-cache';

const getUrl = (version: string, platform: string, arch: string) => `https://github.com/reviewdog/reviewdog/releases/download/v${version}/reviewdog_${version}_${platform}_${arch}.tar.gz`;

interface Release {
	tag_name: string;
}

export async function installReviewdog(tag: string, directory: string): Promise<string> {
	const version = await tagToVersion(tag, 'reviewdog', 'reviewdog');
	const platform = getPlatform === 'darwin' ? 'Darwin' : getPlatform === 'linux' ? 'Linux' : getPlatform === 'win32' ? 'Windows' : '';
	const arch = getArch === 'x64' ? 'x86_64' : getArch === 'arm64' ? getArch : '';

	if (platform === '')
		throw new Error(`unsupported platform ${getPlatform}!`);

	if (arch === '')
		throw new Error(`unsupported architecture: ${getArch}!`);

	const url = getUrl(version, platform, arch);
	const archivePath = await downloadTool(url);
	const extractedDir = await extractTar(archivePath, directory);
	const executablePath = `reviewdog${getPlatform === 'win32' ? '.exe' : ''}`;
	return join(extractedDir, executablePath);
}

async function tagToVersion(tag: string, owner: string, repo: string): Promise<string> {
	const url = `https://github.com/${owner}/${repo}/releases/${tag}`;
	const client = new HttpClient('clippy/v1');
	const headers = { [Headers.Accept]: 'application/json' };
	const response = await client.getJson<Release>(url, headers);

	if (response.statusCode !== HttpCodes.OK)
		throw new Error(`${url} returned unexpected HTTP status code: ${response.statusCode}`);

	if (!response.result)
		throw new Error(`unable to find '${tag}' - use 'latest' or see https://github.com/${owner}/${repo}/releases for details`);

	return response.result.tag_name.replace(/^v/, '');
}

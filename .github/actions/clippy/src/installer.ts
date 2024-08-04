import process from 'node:process';
import { join } from 'pathe';
import { error } from '@actions/core';
import { Headers, HttpClient, HttpCodes } from '@actions/http-client';
import { downloadTool, extractTar } from '@actions/tool-cache';

const getUrl = (version: string, platform: string, arch: string) => `https://github.com/reviewdog/reviewdog/releases/download/v${version}/reviewdog_${version}_${platform}_${arch}.tar.gz`;

interface Release {
	tag_name: string;
}

export async function installReviewdog(tag: string, directory: string): Promise<string> {
	const version = await tagToVersion(tag, 'reviewdog', 'reviewdog');
	const platform = process.platform === 'darwin' ? 'Darwin' : process.platform === 'linux' ? 'Linux' : process.platform === 'win32' ? 'Windows' : '';
	const arch = process.arch === 'x64' ? 'x86_64' : process.arch === 'arm64' ? process.arch : '';

	if (platform === '')
		throw new Error(`unsupported platform ${process.platform}!`);

	if (arch === '')
		throw new Error(`unsupported architecture: ${process.arch}!`);

	const url = getUrl(version, platform, arch);
	const archivePath = await downloadTool(url);
	const extractedDir = await extractTar(archivePath, directory);
	const executablePath = `reviewdog${process.platform === 'win32' ? '.exe' : ''}`;
	return join(extractedDir, executablePath);
}

async function tagToVersion(tag: string, owner: string, repo: string): Promise<string> {
	const url = `https://github.com/${owner}/${repo}/releases/${tag}`;
	const client = new HttpClient('clippy/v1');
	const headers = { [Headers.Accept]: 'application/json' };
	const response = await client.getJson<Release>(url, headers);

	if (response.statusCode !== HttpCodes.OK)
		error(`${url} returned unexpected HTTP status code: ${response.statusCode}`);

	if (!response.result)
		throw new Error(`unable to find '${tag}' - use 'latest' or see https://github.com/${owner}/${repo}/releases for details`);

	return response.result.tag_name.replace(/^v/, '');
}

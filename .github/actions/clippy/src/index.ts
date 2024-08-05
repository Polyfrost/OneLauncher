import os from 'node:os';
import { promises as fs } from 'node:fs';
import process from 'node:process';
import { Buffer } from 'node:buffer';
import { join, relative } from 'pathe';
import { rmRF } from '@actions/io';
import { exec } from '@actions/exec';
import core from '@actions/core';
import { installReviewdog } from './installer';

async function run(): Promise<void> {
	const runnerTmpdir = process.env.RUNNER_TEMP || os.tmpdir();
	const tmpdir = await fs.mkdtemp(join(runnerTmpdir, 'reviewdog-'));

	try {
		const reviewdogVersion = core.getInput('reviewdog_version') || 'latest';
		const toolName = core.getInput('tool_name') || 'clippy';
		const clippyFlags = core.getInput('clippy_flags');
		const clippyDebug = core.getInput('clippy_debug') || 'false';
		const level = core.getInput('level') || 'error';
		const reporter = core.getInput('reporter') || 'github-pr-check';
		const filterMode = core.getInput('filter_mode') || 'added';
		const failOnError = core.getInput('fail_on_error') || 'false';
		const reviewdogFlags = core.getInput('reviewdog_flags');
		const workdir = core.getInput('workdir') || '.';
		const cwd = relative(process.env.GITHUB_WORKSPACE || process.cwd(), workdir);
		const reviewdog = await core.group('installing reviewdog', async () => await installReviewdog(reviewdogVersion, tmpdir));

		const code = await core.group(
			'running clippy',
			async (): Promise<number> => {
				const output: string[] = [];
				await exec(
					'cargo',
					[
						'clippy',
						'--color',
						'never',
						'-q',
						'--message-format',
						'json',
						...parse(clippyFlags),
					],
					{
						cwd,
						ignoreReturnCode: true,
						silent: clippyDebug !== 'true',
						listeners: {
							stdline: (line: string) => {
								const content: CompilerMessage = JSON.parse(line);

								if (content.reason !== 'compiler-message') {
									core.debug('ignore all but `compiler-message`');
									return;
								}

								if (content.message.code === null) {
									core.debug('message code is missing, ignore it');
									return;
								}

								const span = content.message.spans[0]!;
								const messageLevel = content.message.level === 'warning' ? 'w' : 'e';
								const rendered = reporter === 'github-pr-review'
									? ` \n<pre><code>${content.message.rendered}</code></pre>\n__END__`
									: `${content.message.rendered}\n__END__`;
								const ret = `${span.file_name}:${span.line_start}:${span.column_start}:${messageLevel}:${rendered}`;
								output.push(ret);
							},
						},
					},
				);

				process.env.REVIEWDOG_GITHUB_API_TOKEN = core.getInput('github_token');
				return await exec(
					reviewdog,
					[
						'-efm=<pre><code>%E%f:%l:%c:%t:%m',
						'-efm=%E%f:%l:%c:%t:%m',
						'-efm=%Z__END__',
						'-efm=%C%m</code></pre>',
						'-efm=%C%m',
						'-efm=%C',
						`-name=${toolName}`,
						`-reporter=${reporter}`,
						`-filter-mode=${filterMode}`,
						`-fail-on-error=${failOnError}`,
						`-level=${level}`,
						...parse(reviewdogFlags),
					],
					{
						cwd,
						input: Buffer.from(output.join('\n'), 'utf-8'),
						ignoreReturnCode: true,
					},
				);
			},
		);

		if (code !== 0)
			throw new Error(`reviewdog exited with status code: ${code}`);
	}
	catch (error) {
		if (error instanceof Error)
			throw error;
	}
	finally {
		try {
			await rmRF(tmpdir);
		}
		catch (error) {
			if (error instanceof Error)
				core.info(`cleanup failed: ${error.message}`);
			else
				core.info(`cleanup failed: ${error}`);
		}
	}
}

function parse(flags: string): string[] {
	flags = flags.trim();
	if (flags === '')
		return [];

	return flags.split(/\s+/);
}

interface CompilerMessage {
	reason: string;
	message: {
		code: Code;
		level: string;
		message: string;
		rendered: string;
		spans: Span[];
	};
}

interface Code {
	code: string;
	explanation?: string;
}

interface Span {
	file_name: string;
	is_primary: boolean;
	line_start: number;
	line_end: number;
	column_start: number;
	column_end: number;
}

run();

import { tmpdir as getTmpdir } from 'node:os';
import { promises as fs } from 'node:fs';
import { env, cwd as getCwd } from 'node:process';
import { Buffer } from 'node:buffer';
import { join, relative } from 'pathe';
import { rmRF } from '@actions/io';
import { exec } from '@actions/exec';
import { getInput as getCoreInput, group } from '@actions/core';
import { installReviewdog } from './installer';

async function run(): Promise<void> {
	const runnerTmpdir = env.RUNNER_TEMP || getTmpdir();
	const tmpdir = await fs.mkdtemp(join(runnerTmpdir, 'reviewdog-'));
	const getInput = <T = string>(name: string): T => getCoreInput(name) as T;

	const reviewdogVersion = getInput('reviewdog_version') || 'latest';
	const toolName = getInput('tool_name') || 'clippy';
	const clippyFlags = getInput('clippy_flags');
	const clippyDebug = getInput('clippy_debug') || 'false';
	const level = getInput('level') || 'error';
	const reporter = getInput('reporter') || 'github-pr-check';
	const filterMode = getInput('filter_mode') || 'added';
	const failOnError = getInput('fail_on_error') || 'false';
	const reviewdogFlags = getInput('reviewdog_flags');
	const workdir = getInput('workdir') || '.';
	const cwd = relative(env.GITHUB_WORKSPACE || getCwd(), workdir);
	const reviewdog = await group('installing reviewdog', async () => await installReviewdog(reviewdogVersion, tmpdir));

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
				stdline: (data) => {
					const content: CompilerMessage = JSON.parse(data);

					if (content.reason !== 'compiler-message')
						return;

					if (content.message.code === null)
						return;

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

	env.REVIEWDOG_GITHUB_API_TOKEN = getInput('github_token');
	await exec(
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

	await rmRF(tmpdir);
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

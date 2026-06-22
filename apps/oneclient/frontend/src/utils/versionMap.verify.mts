/* eslint-disable no-console, node/prefer-global/process -- standalone dev verification script, not app code */
/**
 * Standalone verification for {@link ./versionMap.ts} parsing/formatting.
 *
 * No test runner is wired into this app, so this is a runnable assertion script:
 *   npx tsx src/utils/versionMap.verify.mts
 *
 * Confirms (Bug 3):
 *   1. OneClient parses the new year-based version "26.1.2" (and "26.1") correctly.
 *   2. No code path emits the malformed "1.26" string for a 2026-era version.
 */
import { formatMcVersion, parseMcVersion } from './versionMap.ts';

let failures = 0;
function check(label: string, actual: unknown, expected: unknown): void {
	const a = JSON.stringify(actual);
	const e = JSON.stringify(expected);
	if (a === e) {
		console.log(`  ok   ${label}`);
	}
	else {
		failures++;
		console.error(`  FAIL ${label}\n         expected ${e}\n         actual   ${a}`);
	}
}

console.log('parseMcVersion:');
check('26.1.2 -> full parse with patch', parseMcVersion('26.1.2'), { major: 26, minor: 1, patch: 2 });
check('26.1   -> no patch', parseMcVersion('26.1'), { major: 26, minor: 1, patch: undefined });
check('26.1.1 -> patch 1', parseMcVersion('26.1.1'), { major: 26, minor: 1, patch: 1 });
check('1.21.5 -> old scheme', parseMcVersion('1.21.5'), { major: 21, minor: 5, patch: undefined });
check('1.21.1 -> old scheme', parseMcVersion('1.21.1'), { major: 21, minor: 1, patch: undefined });
check('null   -> undefined', parseMcVersion(null), undefined);
check('"26"   -> undefined (no minor)', parseMcVersion('26'), undefined);

console.log('formatMcVersion:');
check('(26,1,2) -> 26.1.2', formatMcVersion(26, 1, 2), '26.1.2');
check('(26,1)   -> 26.1', formatMcVersion(26, 1), '26.1');
check('(21,5)   -> 1.21.5', formatMcVersion(21, 5), '1.21.5');

console.log('round-trip (must never produce "1.26"):');
for (const v of ['26.1', '26.1.2', '26.1.1']) {
	const p = parseMcVersion(v)!;
	const out = formatMcVersion(p.major, p.minor!, p.patch);
	check(`${v} round-trips to ${out}`, out, v);
	check(`${v} never yields a "1." prefix`, out.startsWith('1.'), false);
}

if (failures > 0) {
	console.error(`\n${failures} check(s) FAILED`);
	process.exit(1);
}
console.log('\nAll version parsing checks passed.');

import { defineWorkspace } from 'vitest/config';

export default defineWorkspace([
	'packages/client/vitest.config.ts',
	'apps/frontend/vitest.config.ts',
]);

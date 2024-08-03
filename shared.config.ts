import { defineProject as defineVitestConfig } from 'vitest/config';

export function shared(name: TemplateStringsArray) {
	return {
		vitest: defineVitestConfig({
			test: {
				globals: true,
				name: name[0]!,
			},
		}),
	};
}

export default shared;

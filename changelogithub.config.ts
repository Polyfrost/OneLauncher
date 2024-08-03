// define the `changelogithub` configuration so we can automatically generate
// changelogs with github actions upon each `pnpm release`.
export default {
	types: {
		chore: { title: '🔨 Chores' },
		feature: { title: '✨ Features' },
		fix: { title: '🐞 Bug Fixes' },
		perf: { title: '🏎 Performance' },
		refactor: { title: '♻️ Refactors' },
		test: { title: '✅ Tests' },
		style: { title: '🎨 Stylistic' },
		doc: { title: '📝 Docs' },
		deps: { title: '📦 Dependencies' },
		deploy: { title: '🚀 Deployments' },
		wip: { title: '🚧 Experiments' },
	},
	capitalize: false,
};

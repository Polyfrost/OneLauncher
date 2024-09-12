import hljs from 'highlight.js/lib/core';
import { configuredXss, md, noop } from './markdown';

function langFactory(lang: string) {
	return (imp: typeof import('highlight.js/lib/languages/*')) => {
		hljs.registerLanguage(lang, imp.default);
	};
}

export function registerLanguages() {
	const _languages = [
		import('highlight.js/lib/languages/javascript').then(langFactory('javascript')),
		import('highlight.js/lib/languages/python').then(langFactory('python')),
		import('highlight.js/lib/languages/lua').then(langFactory('lua')),
		import('highlight.js/lib/languages/kotlin').then(langFactory('kotlin')),
		import('highlight.js/lib/languages/scala').then(langFactory('scala')),
		import('highlight.js/lib/languages/groovy').then(langFactory('groovy')),
		import('highlight.js/lib/languages/gradle').then(langFactory('gradle')),
		import('highlight.js/lib/languages/json').then(langFactory('json')),
		import('highlight.js/lib/languages/ini').then(langFactory('ini')),
		import('highlight.js/lib/languages/yaml').then(langFactory('yaml')),
		import('highlight.js/lib/languages/properties').then(langFactory('properties')),
	];
}

registerLanguages();
hljs.registerAliases(['js'], { languageName: 'javascript' });
hljs.registerAliases(['py'], { languageName: 'python' });
hljs.registerAliases(['kt', 'kts'], { languageName: 'kotlin' });
hljs.registerAliases(['json5'], { languageName: 'json' });
hljs.registerAliases(['toml'], { languageName: 'ini' });
hljs.registerAliases(['yml'], { languageName: 'yaml' });
hljs.registerAliases(['html', 'htm', 'xhtml', 'mcui', 'fxml'], { languageName: 'xml' });

export function renderHighlightedString(src: string): string {
	return configuredXss.process(
		md({
			highlight(str, lang) {
				if (lang && hljs.getLanguage(lang))
					try {
						return hljs.highlight(str, { language: lang }).value;
					}
					catch (err) {
						noop(err);
					}

				return '';
			},
		}).render(src),
	);
}

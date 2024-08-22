import hljs from 'highlight.js/lib/core';
import { configuredXss, md, noop } from './markdown';

['javascript', 'python', 'lua', 'java', 'kotlin', 'scala', 'groovy', 'gradle', 'json', 'ini', 'yaml', 'xml', 'properties'].forEach(async (name) => {
	hljs.registerLanguage(name, (await import(`highlight.js/lib/languages/${name}`)).default);
});

hljs.registerAliases(['js'], { languageName: 'javascript' });
hljs.registerAliases(['py'], { languageName: 'python' });
hljs.registerAliases(['kt'], { languageName: 'kotlin' });
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

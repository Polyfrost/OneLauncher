import hljs from 'highlight.js/lib/core';
import { configuredXss, md, noop } from './markdown';

hljs.registerLanguage('javascript', (await import('highlight.js/lib/languages/javascript')).default);
hljs.registerLanguage('python', (await import('highlight.js/lib/languages/python')).default);
hljs.registerLanguage('lua', (await import('highlight.js/lib/languages/lua')).default);
hljs.registerLanguage('java', (await import('highlight.js/lib/languages/java')).default);
hljs.registerLanguage('kotlin', (await import('highlight.js/lib/languages/kotlin')).default);
hljs.registerLanguage('scala', (await import('highlight.js/lib/languages/scala')).default);
hljs.registerLanguage('groovy', (await import('highlight.js/lib/languages/groovy')).default);
hljs.registerLanguage('gradle', (await import('highlight.js/lib/languages/gradle')).default);
hljs.registerLanguage('json', (await import('highlight.js/lib/languages/json')).default);
hljs.registerLanguage('ini', (await import('highlight.js/lib/languages/ini')).default);
hljs.registerLanguage('yaml', (await import('highlight.js/lib/languages/yaml')).default);
hljs.registerLanguage('xml', (await import('highlight.js/lib/languages/xml')).default);
hljs.registerLanguage('properties', (await import('highlight.js/lib/languages/properties')).default);
hljs.registerAliases(['js'], { languageName: 'javascript' });
hljs.registerAliases(['py'], { languageName: 'python' });
hljs.registerAliases(['kt'], { languageName: 'kotlin' });
hljs.registerAliases(['json5'], { languageName: 'json' });
hljs.registerAliases(['toml'], { languageName: 'ini' });
hljs.registerAliases(['yml'], { languageName: 'yaml' });
hljs.registerAliases(['html', 'htm', 'xhtml', 'mcui', 'fxml'], { languageName: 'xml' });

export function renderHighlightedString(src: string) {
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

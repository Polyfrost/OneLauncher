import hljs from 'highlight.js/lib/core';
import javascript from 'highlight.js/lib/languages/javascript';
import python from 'highlight.js/lib/languages/python';
import lua from 'highlight.js/lib/languages/lua';
import java from 'highlight.js/lib/languages/java';
import kotlin from 'highlight.js/lib/languages/kotlin';
import scala from 'highlight.js/lib/languages/scala';
import groovy from 'highlight.js/lib/languages/groovy';
import gradle from 'highlight.js/lib/languages/gradle';
import json from 'highlight.js/lib/languages/json';
import ini from 'highlight.js/lib/languages/ini';
import yaml from 'highlight.js/lib/languages/yaml';
import xml from 'highlight.js/lib/languages/xml';
import properties from 'highlight.js/lib/languages/properties';
import { configuredXss, md } from './markdown';

hljs.registerLanguage('javascript', javascript);
hljs.registerLanguage('python', python);
hljs.registerLanguage('lua', lua);
hljs.registerLanguage('java', java);
hljs.registerLanguage('kotlin', kotlin);
hljs.registerLanguage('scala', scala);
hljs.registerLanguage('groovy', groovy);
hljs.registerLanguage('gradle', gradle);
hljs.registerLanguage('json', json);
hljs.registerLanguage('ini', ini);
hljs.registerLanguage('yaml', yaml);
hljs.registerLanguage('xml', xml);
hljs.registerLanguage('properties', properties);
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

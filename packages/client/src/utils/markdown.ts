import MarkdownIt, { type Options as MarkdownOptions } from 'markdown-it';
import { FilterXSS, escapeAttrValue, safeAttrValue, whiteList } from 'xss';

// from https://github.com/modrinth/code/blob/main/packages/utils/parse.ts for parsing modrinth style markdown
export const configuredXss = new FilterXSS({
	whiteList: {
		...whiteList,
		summary: [],
		h1: ['id'],
		h2: ['id'],
		h3: ['id'],
		h4: ['id'],
		h5: ['id'],
		h6: ['id'],
		kbd: ['id'],
		input: ['checked', 'disabled', 'type'],
		iframe: ['width', 'height', 'allowfullscreen', 'frameborder', 'start', 'end'],
		img: [...(whiteList.img || []), 'usemap', 'style'],
		map: ['name'],
		area: [...(whiteList.a || []), 'coords'],
		a: [...(whiteList.a || []), 'rel'],
		td: [...(whiteList.td || []), 'style'],
		th: [...(whiteList.th || []), 'style'],
		picture: [],
		source: ['media', 'sizes', 'src', 'srcset', 'type'],
	},
	css: {
		whiteList: {
			'image-rendering': /^pixelated$/,
			'text-align': /^center|left|right$/,
			'float': /^left|right$/,
		},
	},
	onIgnoreTagAttr: (tag, name, value) => {
		if (tag === 'iframe' && name === 'src') {
			const allowed = [
				{
					url: /^https?:\/\/(www\.)?youtube(-nocookie)?\.com\/embed\/[\w-]{11}/,
					allowedParameters: [/start=\d+/, /end=\d+/],
				},
				{
					url: /^https?:\/\/(www\.)?discord\.com\/widget/,
					allowedParameters: [/id=\d{18,19}/],
				},
			];

			const url = new URL(value);
			for (const src of allowed) {
				if (!src.url.test(url.href))
					continue;

				const newSearchParams = new URLSearchParams();
				url.searchParams.forEach((v, k) => {
					if (!src.allowedParameters.some(p => p.test(`${k}=${v}`)))
						newSearchParams.delete(k);
				});

				url.search = newSearchParams.toString();
				return `${name}="${escapeAttrValue(url.toString())}"`;
			}
		}

		if (name === 'class' && ['pre', 'code', 'span'].includes(tag)) {
			const allowedClasses: string[] = [];
			for (const className of value.split(/\s/g))
				if (className.startsWith('hljs-') || className.startsWith('language-'))
					allowedClasses.push(className);

			return `${name}="${escapeAttrValue(allowedClasses.join(' '))}"`;
		}

		return undefined;
	},
	safeAttrValue: (tag, name, value, cssFilter) => {
		if (tag === 'img' && name === 'src' && !value.startsWith('data:'))
			try {
				const url = new URL(value);
				if (url.hostname.includes('wsrv.nl'))
					url.searchParams.delete('errorredirect');

				// TODO: do we need all this lol...
				// TODO: add cloudflare stuff if needed
				const allowed = [
					'imgur.com',
					'i.imgur.com',
					'cdn-raw.modrinth.com',
					'cdn.modrinth.com',
					'staging-cdn-raw.modrinth.com',
					'staging-cdn.modrinth.com',
					'github.com',
					'raw.githubusercontent.com',
					'img.shields.io',
					'i.postimg.cc',
					'wsrv.nl',
					'cf.way2muchnoise.eu',
					'bstats.org',
				];

				if (!allowed.includes(url.hostname))
					return safeAttrValue(
						tag,
						name,
						`https://wsrv.nl/?url=${encodeURIComponent(
							url.toString().replaceAll('&amp;', '&'),
						)}&n=-1`,
						cssFilter,
					);

				return safeAttrValue(tag, name, url.toString(), cssFilter);
			}
			catch (err) {
				noop(err);
			}

		return safeAttrValue(tag, name, value, cssFilter);
	},
});

export function md(options: MarkdownOptions = {}) {
	const md = new MarkdownIt('default', {
		html: true,
		linkify: true,
		breaks: false,
		...options,
	});

	const defaultLinkOpenRenderer
		= md.renderer.rules.link_open || ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));

	md.renderer.rules.link_open = (tokens, idx, options, env, self) => {
		const token = tokens[idx]!;
		const index = token.attrIndex('href');

		if (token.attrs && index !== -1) {
			const href = token.attrs[index]![1];

			try {
				const url = new URL(href);
				const allowed = ['modrinth.com'];
				if (allowed.includes(url.hostname))
					return defaultLinkOpenRenderer(tokens, idx, options, env, self);
			}
			catch (err) {
				noop(err);
			}
		}

		tokens[idx]!.attrSet('rel', 'noopener nofollow ugc');
		return defaultLinkOpenRenderer(tokens, idx, options, env, self);
	};

	return md;
}

export const renderString = (src: string) => configuredXss.process(md().render(src));

export function noop(_t: any) {}

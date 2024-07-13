declare namespace globalThis {
    export type WithIndex<T> = T & { index: number }
    export type MakeOptional<T, K extends keyof T> = Omit<T, K> & Partial<Pick<T, K>>
}

// TODO remove in place for specta-generated types
interface Mod {
    id: string;
	name: string;
	description: string;
	author: string;
	icon_url: string;
	provider: Provider;
	page_url: string;
	downloads: number;
	ratings: number;
}
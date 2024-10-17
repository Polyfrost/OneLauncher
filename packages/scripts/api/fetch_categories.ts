import process from 'node:process';
import { findWorkspaceDir } from '@pnpm/find-workspace-dir';
import dotenv from 'dotenv';

interface MappedCategoryItem {
	display: string;
	id: string;
}

type MappingList = Record<string, MappedCategoryItem[]>;

const packageTypes = ['mod', 'modpack', 'shader', 'resourcepack'] as const;
type PackageType = typeof packageTypes[number];

async function loadEnv() {
	const path = await findWorkspaceDir(process.cwd());
	console.log(`Loading .env from ${path}`);
	dotenv.config({ path: `${path}/.env` });
}

async function fetchCategoriesCurseforge(packageType: PackageType): Promise<MappingList> {
	const key = process.env.CURSEFORGE_API_KEY;

	if (key === undefined) {
		console.error('CURSEFORGE_API_KEY is not defined');
		return {};
	}

	const response = await fetch('https://api.curseforge.com/v1/categories?gameId=432', {
		headers: {
			'x-api-key': key,
		},
	});

	interface CFCategory {
		id: number;
		name: 'Server Utility';
		slug: 'server-utility';
		url: 'https://www.curseforge.com/minecraft/mc-mods/server-utility';
		iconUrl: 'https://media.forgecdn.net/avatars/6/48/635351498950580836.png';
		classId?: number;
		isClass?: boolean;
		parentCategoryId?: number;
	};

	const data = (await response.json() as { data: CFCategory[] }).data;

	// TODO: Add more calssIds
	const classIds: Record<PackageType, number> = {
		mod: 6,
		resourcepack: 12,
		modpack: 4471,
		shader: 6552,
	};

	const mapping: MappingList = {};

	mapping.Curseforge = data
		.filter((category) => {
			return category.isClass !== true && category.classId === classIds[packageType];
		})
		.map((category) => {
			return {
				display: category.name,
				id: category.id.toString(),
			};
		});

	return mapping;
}

async function fetchCategoriesModrinth(packageType: PackageType): Promise<MappingList> {
	const response = await fetch('https://api.modrinth.com/v2/tag/category');

	interface ModrinthCategory {
		icon: string;
		name: string;
		project_type: PackageType;
		header: string;
	}

	const data = await response.json() as ModrinthCategory[];

	const mapping: MappingList = {};

	mapping.Modrinth = data
		.filter((category) => {
			return category.project_type === packageType;
		})
		.map((category) => {
			return {
				display: category.name.charAt(0).toUpperCase() + category.name.slice(1),
				id: category.name,
			};
		});

	return mapping;
}

(async () => {
	const arg = process.argv[2];
	if (arg === undefined) {
		console.error(`Please provide an argument for package type.\n Valid: '${packageTypes.join('\', \'')}'`);
		return;
	}

	const packageType = arg as PackageType;

	await loadEnv();

	const cfCategories = await fetchCategoriesCurseforge(packageType);
	const modrinthCategories = await fetchCategoriesModrinth(packageType);

	const categories = {
		...cfCategories,
		...modrinthCategories,
	};

	console.log(categories);
})();

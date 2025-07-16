import type { GameLoader } from '@/bindings.gen';

export function prettifyLoader(loader: GameLoader): string {
	switch (loader) {
		case 'fabric':
			return 'Fabric';
		case 'forge':
			return 'Forge';
		case 'neoforge':
			return 'NeoForge';
		case 'quilt':
			return 'Quilt';
		case 'vanilla':
			return 'Vanilla';
		case 'legacyfabric':
			return 'Legacy Fabric';
	}
}

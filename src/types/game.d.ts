declare namespace game {
	export interface GameClientDetails {
		uuid: Uuid;
		name: string;
		version: string;
		main_class: string;
		java_version: JavaVersion;
		startup_args: Vec<string>;
		client_type: {
			type: keyof GameClientType;
			manifest: GameClientType[keyof GameClientType];
		};
	}

	export interface GameClientType {
		Vanilla: VanillaManifest;
	}

	export type ClientType = keyof GameClientType;

	export interface VanillaManifest {}
}

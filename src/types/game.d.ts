declare namespace Core {
    export interface InstanceWithManifest {
        instance: Instance,
        manifest: Manifest,
    }

    export interface Manifest {
        id: string,
        manifest: MinecraftManifest
    }

    export interface MinecraftManifest {
        id: string,
        javaVersion: {
            majorVersion: number;
        },
    }

	export interface Instance<T extends keyof ClientType = keyof ClientType> {
        id: string,
        createdAt: number,
        name: string,
        cover: string | null,
        group: string | null,
        client: ClientType[T],
    }

    export interface ClientType {
        Vanilla: VanillaProps,
    }

    export interface VanillaProps {
        type: "Vanilla",
    }
}

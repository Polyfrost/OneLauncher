declare namespace Core {
    export type ClusterWithManifest<T extends keyof ClientType = keyof ClientType> = {
        cluster: Cluster<T>,
        manifest: Manifest
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

	export interface Cluster<T extends keyof ClientType = keyof ClientType> {
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

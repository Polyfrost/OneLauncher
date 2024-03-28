declare namespace Core {
    export interface ClusterWithManifest {
        cluster: Cluster,
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

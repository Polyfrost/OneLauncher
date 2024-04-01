declare namespace globalThis {
    export type WithIndex<T> = T & { index: number }
}
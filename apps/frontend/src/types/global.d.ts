declare namespace globalThis {
    export type WithIndex<T> = T & { index: number }
    export type MakeOptional<T, K extends keyof T> = Omit<T, K> & Partial<Pick<T, K>>
    
    type GrowToSize<T, N extends number, A extends T[]> = A['length'] extends N ? A : GrowToSize<T, N, [...A, T]>;
    export type FixedArray<T, N extends number> = GrowToSize<T, N, []>;

    // Get size of enum as a type - https://stackoverflow.com/a/68317984/23592613
    type UnionToIntersection<U> = (U extends any ? (k: U) => void : never) extends (k: infer I) => void ? I : never;
    type UnionToOvlds<U> = UnionToIntersection<U extends any ? (f: U) => void : never>;
    type PopUnion<U> = UnionToOvlds<U> extends (a: infer A) => void ? A : never;
    export type IsUnion<T> = [T] extends [UnionToIntersection<T>] ? false : true;
    export type UnionToArray<T, A extends unknown[] = []> = IsUnion<T> extends true ? UnionToArray<Exclude<T, PopUnion<T>>, [PopUnion<T>, ...A]> : [T, ...A];

    // enum Example {
    //     how = "how",
    //     to = "to",
    //     count = "count",
    //     enum = "enum",
    //     entries = "entries"
    // }

    // type Result = UnionToArray<keyof typeof Example>['length'] // = 5
}
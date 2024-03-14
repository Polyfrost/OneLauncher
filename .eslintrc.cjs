module.exports = {
    "root": true,
    "extends": [
        "eslint:recommended",
        "plugin:@typescript-eslint/recommended",
        "plugin:solid/typescript",
        "airbnb-base",
        "airbnb-typescript/base",
    ],
    "parser": "@typescript-eslint/parser",
    "parserOptions": { "project": ["./tsconfig.json"] },
    "plugins": [
        "@typescript-eslint",
        "@stylistic/ts",
        "solid"
    ],
    "rules": {
        "indent": "off",
        "@typescript-eslint/indent": "off",
        "@stylistic/ts/indent": ['error', 4],
        "@typescript-eslint/no-unused-vars": "warn",
        "import/extensions": [
            "error",
            {
                "ts": "never",
                "tsx": "never"
            }
        ]
    },
    "ignorePatterns": [
        ".eslintrc.cjs"
    ]
}
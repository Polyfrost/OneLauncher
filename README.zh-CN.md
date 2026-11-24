# OneClient / OneLauncher 简体中文汉化

本分支为 [Polyfrost/OneLauncher](https://github.com/Polyfrost/OneLauncher) 的当前 Rust / Freya 界面添加内置简体中文。汉化源码维护于 [diaper23/OneLauncher](https://github.com/diaper23/OneLauncher)。

## 使用

使用本分支构建的程序，在首次启动的语言选择页或 **Settings → Language（设置 → 语言）** 中选择 **简体中文**，无需联网下载语言包。以后启动会记住语言选择。

已有英文版安装程序不会因为 Fork 更新而自动获得本分支的改动，需要使用从本分支构建的程序。

## 汉化内容

- 覆盖原有 2700 条英文文案，并补齐主题编辑器、外观预览和反馈页的 45 条文案，共 2745 条。
- 包含首页、实例管理、模组、整合包、资源包、光影、世界、截图、日志、账号登录、社交、衣柜、设置、更新提示和模组诊断。
- 品牌名、模组名、版本号、文件名、代码示例及远程提供的项目说明保持原样；开发者调试页、上游更新日志及远程服务返回的原始错误不属于语言包译文。
- 使用大陆常见术语，如“整合包”“资源包”“光影”“内存”“剪贴板”。OneClient 的 bundles 译为“模组组合”，与 modpacks（整合包）区分。
- 为 Windows、macOS 和 Linux 补充中文系统字体回退。Linux 如出现缺字，请安装 Noto Sans CJK SC 或 Noto Sans SC。

## 构建

需要 Rust 1.97 或更新版本及相应平台的构建工具。Windows 建议使用 MSVC 工具链，并安装 Visual Studio Build Tools 的“使用 C++ 的桌面开发”组件。其他平台请参阅 [上游构建说明](CONTRIBUTING.md)。

```sh
git clone --branch feat/zh-cn https://github.com/diaper23/OneLauncher.git
cd OneLauncher
cargo run -p oneclient_app
# 构建正式版
cargo build --release -p oneclient_app
```

打包安装程序的方法见 [README](README.md#packaging--releasing)。

## 维护与检查

英文源文案：[`packages/oneclient_app/lang/en_us.json`](packages/oneclient_app/lang/en_us.json)。
简中译文：[`packages/oneclient_app/lang/zh_cn.json`](packages/oneclient_app/lang/zh_cn.json)。

新增界面文案时，先通过 `t` 或 `tf` 引用翻译键，再在两份 JSON 中添加相同的键。不要修改 `%s`、`%1$s` 等变量占位符及 `%%` 转义百分号。必要时可以使用带编号的占位符调整中文语序。

```sh
python .github/crowdin/check_translations.py
cargo test -p oneclient_app --lib translations::tests --locked
cargo fmt --all -- --check
cargo clippy -p oneclient_app --lib --locked
```

内置语言包保证首次启动和离线时可用。在线社区译文中通过校验的条目会覆盖对应内置译文；缺少或格式错误的条目继续使用内置中文，其他语言的行为保持原有逻辑。

## 来源与致谢

1910 条译文参考官方社区繁中语言包，经简化字转换、大陆用语调整和术语校订；其余 790 条原有文案及 45 条新增文案在本分支补译。感谢 Polyfrost 和社区翻译贡献者。

参考语言包：[官方 zh_TW/oneclient.json](https://translations.r2.polyfrost.org/oneclient/v1/translations/zh_TW/oneclient.json)，对应分发清单时间戳 `1795448544`。代码和译文沿用项目的 [GPL-3.0 许可证](LICENSE)，品牌素材的授权说明见 [ATTRIBUTION.md](ATTRIBUTION.md)。

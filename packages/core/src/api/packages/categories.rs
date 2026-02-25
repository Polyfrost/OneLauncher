use onelauncher_entity::package::PackageType;
use serde::{Deserialize, Serialize};

use crate::api::packages::provider::ProviderExt;

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageCategories {
	Mod(Vec<PackageModCategory>),
	ResourcePack(Vec<PackageResourcePackCategory>),
	Shader(Vec<PackageShaderCategory>),
	DataPack(Vec<PackageModCategory>),
	ModPack(Vec<PackageModPackCategory>),
}

pub trait ToProviderCategory<Out, Provider>
where
	Provider: ProviderExt,
{
	fn as_mod_out(category: &PackageModCategory) -> Out;
	fn to_mod(value: &Out) -> Option<PackageModCategory>;

	fn as_resource_pack_out(category: &PackageResourcePackCategory) -> Out;
	fn to_resource_pack(value: &Out) -> Option<PackageResourcePackCategory>;

	fn as_shader_out(category: &PackageShaderCategory) -> Out;
	fn to_shader(value: &Out) -> Option<PackageShaderCategory>;

	fn as_mod_pack_out(category: &PackageModPackCategory) -> Out;
	fn to_mod_pack(value: &Out) -> Option<PackageModPackCategory>;

	#[must_use]
	fn as_data_pack_out(category: &PackageModCategory) -> Out {
		Self::as_mod_out(category)
	}

	fn to_data_pack(value: &Out) -> Option<PackageModCategory> {
		Self::to_mod(value)
	}

	fn as_out(categories: &PackageCategories) -> Vec<Out> {
		match categories {
			PackageCategories::Mod(categories) => categories.iter().map(Self::as_mod_out).collect(),
			PackageCategories::ResourcePack(categories) => {
				categories.iter().map(Self::as_resource_pack_out).collect()
			}
			PackageCategories::Shader(categories) => {
				categories.iter().map(Self::as_shader_out).collect()
			}
			PackageCategories::DataPack(categories) => {
				categories.iter().map(Self::as_data_pack_out).collect()
			}
			PackageCategories::ModPack(categories) => {
				categories.iter().map(Self::as_mod_pack_out).collect()
			}
		}
	}

	#[must_use]
	fn to_list(package_type: &PackageType, values: &[Out]) -> PackageCategories {
		match *package_type {
			PackageType::Mod => {
				PackageCategories::Mod(values.iter().filter_map(|v| Self::to_mod(v)).collect())
			}
			PackageType::ResourcePack => PackageCategories::ResourcePack(
				values
					.iter()
					.filter_map(|v| Self::to_resource_pack(v))
					.collect(),
			),
			PackageType::Shader => PackageCategories::Shader(
				values.iter().filter_map(|v| Self::to_shader(v)).collect(),
			),
			PackageType::DataPack => PackageCategories::DataPack(
				values
					.iter()
					.filter_map(|v| Self::to_data_pack(v))
					.collect(),
			),
			PackageType::ModPack => PackageCategories::ModPack(
				values.iter().filter_map(|v| Self::to_mod_pack(v)).collect(),
			),
		}
	}
}

macro_rules! define_category_mapping {
    (
        impl $trait_name:ident <$out:ty, $provider:ty> for $struct_name:ty {
            $(
                $enum_type:ty as $fn_name:ident => $reverse_fn:ident {
                    $($variant:ident => $value:expr),* $(,)?
                }
            )*
        }
    ) => {
        impl $trait_name<$out, $provider> for $struct_name {
            $(
                fn $fn_name(category: &$enum_type) -> $out {
                    match category {
                        $(
                            <$enum_type>::$variant => $value,
                        )*
                    }
                }
            )*

			$(
				fn $reverse_fn(value: &$out) -> Option<$enum_type> {
					match value {
						$(
							v if v == &$value => Some(<$enum_type>::$variant),
						)*
						_ => None,
					}
				}
			)*
        }
    };
}

pub(super) use define_category_mapping;

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageModCategory {
	Adventure,
	Library,
	Equipment,
	Patches,
	Cosmetic,
	Food,
	Magic,
	Information,
	Misc,
	Performance,
	Redstone,
	ServerUtil,
	Storage,
	Technology,
	Farming,
	Automation,
	Transport,
	Utility,
	QoL,
	WorldGen,
	Mobs,
	Economy,
	Social,
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageResourcePackCategory {
	X8,
	X16,
	X32,
	X48,
	X64,
	X128,
	X256,
	X512,

	VanillaLike,
	Utility,
	Tweaks,
	Themed,
	Simplistic,
	Realistic,
	Modded,
	Decoration,
	Cursed,
	Combat,

	Audio,
	Blocks,
	CoreShaders,
	Gui,
	Fonts,
	Equipment,
	Environment,
	Entities,
	Items,
	Locale,
	Models,
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageShaderCategory {
	VanillaLike,
	SemiRealistic,
	Realistic,
	Fantasy,
	Cursed,
	Cartoon,

	Bloom,
	Atmosphere,
	Reflections,
	Shadows,
	PBR,
	PathTracing,
	Foliage,
	ColoredLightning,

	Potato,
	Low,
	Medium,
	High,
	Ultra,
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageModPackCategory {
	Technology,
	Quests,
	Optimization,
	Multiplayer,
	Magic,
	LightWeight,
	Combat,
	Challenging,
	Adventure,
}

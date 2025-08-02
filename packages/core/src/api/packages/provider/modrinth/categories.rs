use crate::api::packages::categories::{
	PackageModCategory, PackageModPackCategory, PackageResourcePackCategory, PackageShaderCategory,
	ToProviderCategory, define_category_mapping,
};
use crate::api::packages::provider::ModrinthProviderImpl;

pub struct ModrinthCategories;

define_category_mapping! {
	impl ToProviderCategory<String, ModrinthProviderImpl> for ModrinthCategories {
		PackageModCategory as as_mod_out => to_mod {
			Adventure => String::from("adventure"),
			Library => String::from("library"),
			Equipment => String::from("equipment"),
			Patches => String::from("optimization"),
			Cosmetic => String::from("decoration"),
			Food => String::from("food"),
			Magic => String::from("magic"),
			Information => String::from("utility"),
			Misc => String::from("utility"),
			Performance => String::from("optimization"),
			Redstone => String::from("technology"),
			ServerUtil => String::from("management"),
			Storage => String::from("storage"),
			Technology => String::from("technology"),
			Farming => String::from("food"),
			Automation => String::from("technology"),
			Transport => String::from("transportation"),
			Utility => String::from("utility"),
			QoL => String::from("utility"),
			WorldGen => String::from("worldgen"),
			Mobs => String::from("mobs"),
			Economy => String::from("economy"),
			Social => String::from("social"),
		}

		PackageResourcePackCategory as as_resource_pack_out => to_resource_pack {
			X8 => String::from("8x-"),
			X16 => String::from("16x"),
			X32 => String::from("32x"),
			X48 => String::from("48x"),
			X64 => String::from("64x"),
			X128 => String::from("128x"),
			X256 => String::from("256x"),
			X512 => String::from("512x+"),
			VanillaLike => String::from("vanilla-like"),
			Utility => String::from("utility"),
			Tweaks => String::from("tweaks"),
			Themed => String::from("themed"),
			Simplistic => String::from("simplistic"),
			Realistic => String::from("realistic"),
			Modded => String::from("modded"),
			Decoration => String::from("decoration"),
			Cursed => String::from("cursed"),
			Combat => String::from("combat"),
			Audio => String::from("audio"),
			Blocks => String::from("blocks"),
			CoreShaders => String::from("core-shaders"),
			Gui => String::from("gui"),
			Fonts => String::from("fonts"),
			Equipment => String::from("equipment"),
			Environment => String::from("environment"),
			Entities => String::from("entities"),
			Items => String::from("items"),
			Locale => String::from("locale"),
			Models => String::from("models"),
		}

		PackageShaderCategory as as_shader_out => to_shader {
			VanillaLike => String::from("vanilla-like"),
			SemiRealistic => String::from("semi-realistic"),
			Realistic => String::from("realistic"),
			Fantasy => String::from("fantasy"),
			Cursed => String::from("cursed"),
			Cartoon => String::from("cartoon"),
			Bloom => String::from("bloom"),
			Atmosphere => String::from("atmosphere"),
			Reflections => String::from("reflections"),
			Shadows => String::from("shadows"),
			PBR => String::from("pbr"),
			PathTracing => String::from("path-tracing"),
			Foliage => String::from("foliage"),
			ColoredLightning => String::from("colored-lightning"),
			Potato => String::from("potato"),
			Low => String::from("low"),
			Medium => String::from("medium"),
			High => String::from("high"),
			Ultra => String::from("screenshot"),
		}

		PackageModPackCategory as as_mod_pack_out => to_mod_pack {
			Adventure => String::from("adventure"),
			Technology => String::from("technology"),
			Quests => String::from("quests"),
			Optimization => String::from("optimization"),
			Multiplayer => String::from("multiplayer"),
			Magic => String::from("magic"),
			LightWeight => String::from("lightweight"),
			Combat => String::from("combat"),
			Challenging => String::from("challenging"),
		}
	}
}

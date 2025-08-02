use crate::api::packages::categories::{
	PackageModCategory, PackageModPackCategory, PackageResourcePackCategory, PackageShaderCategory,
	ToProviderCategory, define_category_mapping,
};
use crate::api::packages::provider::CurseForgeProviderImpl;

pub struct CurseForgeCategories;

define_category_mapping! {
	impl ToProviderCategory<u32, CurseForgeProviderImpl> for CurseForgeCategories {
		PackageModCategory as as_mod_out => to_mod {
			Adventure => 0,
			Library => 0,
			Equipment => 0,
			Patches => 0,
			Cosmetic => 0,
			Food => 0,
			Magic => 0,
			Information => 0,
			Misc => 0,
			Performance => 0,
			Redstone => 0,
			ServerUtil => 0,
			Storage => 0,
			Technology => 0,
			Farming => 0,
			Automation => 0,
			Transport => 0,
			Utility => 0,
			QoL => 0,
			WorldGen => 0,
			Mobs => 0,
			Economy => 0,
			Social => 0,
		}

		PackageResourcePackCategory as as_resource_pack_out => to_resource_pack {
			X8 => 0,
			X16 => 0,
			X32 => 0,
			X48 => 0,
			X64 => 0,
			X128 => 0,
			X256 => 0,
			X512 => 0,
			VanillaLike => 0,
			Utility => 0,
			Tweaks => 0,
			Themed => 0,
			Simplistic => 0,
			Realistic => 0,
			Modded => 0,
			Decoration => 0,
			Cursed => 0,
			Combat => 0,
			Audio => 0,
			Blocks => 0,
			CoreShaders => 0,
			Gui => 0,
			Fonts => 0,
			Equipment => 0,
			Environment => 0,
			Entities => 0,
			Items => 0,
			Locale => 0,
			Models => 0,
		}

		PackageShaderCategory as as_shader_out => to_shader {
			VanillaLike => 0,
			SemiRealistic => 0,
			Realistic => 0,
			Fantasy => 0,
			Cursed => 0,
			Cartoon => 0,
			Bloom => 0,
			Atmosphere => 0,
			Reflections => 0,
			Shadows => 0,
			PBR => 0,
			PathTracing => 0,
			Foliage => 0,
			ColoredLightning => 0,
			Potato => 0,
			Low => 0,
			Medium => 0,
			High => 0,
			Ultra => 0,
		}

		PackageModPackCategory as as_mod_pack_out => to_mod_pack {
			Adventure => 0,
			Technology => 0,
			Quests => 0,
			Optimization => 0,
			Multiplayer => 0,
			Magic => 0,
			LightWeight => 0,
			Combat => 0,
			Challenging => 0,
		}
	}
}

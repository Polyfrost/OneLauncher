// @generated automatically by Diesel CLI.

diesel::table! {
    cluster_groups (id) {
        id -> Integer,
        name -> Text,
    }
}

diesel::table! {
    clusters (path) {
        path -> Text,
        created_at -> Integer,
        updated_at -> Integer,
        group_id -> Nullable<Integer>,
        name -> Text,
        mc_version -> Text,
        mc_loader -> Text,
        mc_loader_version -> Nullable<Text>,
        last_played -> Nullable<Integer>,
        overall_played -> Nullable<Integer>,
        icon_url -> Nullable<Text>,
        setting_profile_name -> Nullable<Text>,
        linked_pack_id -> Nullable<Text>,
        linked_pack_version -> Nullable<Integer>,
    }
}

diesel::table! {
    java_versions (id) {
        id -> Integer,
        major_version -> Integer,
        display_name -> Text,
        absolute_path -> Text,
        full_version -> Text,
    }
}

diesel::table! {
    packages (file_name, type_id) {
        file_name -> Text,
        display_name -> Text,
        display_version -> Text,
        type_id -> Integer,
        provider_id -> Integer,
        provider_version -> Text,
        mc_versions -> Text,
        mc_loader -> Text,
        hash -> Text,
        icon_url -> Nullable<Text>,
    }
}

diesel::table! {
    setting_profiles (name) {
        name -> Text,
        java_id -> Nullable<Integer>,
        res_w -> Nullable<Integer>,
        res_h -> Nullable<Integer>,
        force_fullscreen -> Nullable<Integer>,
        mem_max -> Nullable<Integer>,
        launch_args -> Nullable<Text>,
        launch_env -> Nullable<Text>,
        hook_pre -> Nullable<Text>,
        hook_wrapper -> Nullable<Text>,
        hook_post -> Nullable<Text>,
    }
}

diesel::joinable!(clusters -> cluster_groups (group_id));
diesel::joinable!(clusters -> setting_profiles (setting_profile_name));
diesel::joinable!(setting_profiles -> java_versions (java_id));

diesel::allow_tables_to_appear_in_same_query!(
    cluster_groups,
    clusters,
    java_versions,
    packages,
    setting_profiles,
);

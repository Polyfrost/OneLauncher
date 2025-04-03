create table cluster_groups (
	id integer not null,
	name text not null,
	constraint cluster_groups_pk primary key (id autoincrement)
);

create table java_versions (
	id integer not null,
	major_version integer not null,
	vendor_name text not null,
	absolute_path text not null,
	full_version text not null,
	constraint java_versions_pk primary key (id autoincrement)
);

create index java_versions_major_version_idx on java_versions (major_version);

create table packages (
	hash text not null,
	file_name text not null,
	display_name text not null,
	display_version text not null,
	type_id integer not null,
	provider_id integer not null,
	provider_version text not null,
	mc_versions text not null,
	mc_loader text not null,
	icon_url text,
	constraint packages_pk primary key (hash)
);

create table setting_profiles (
	name text not null,
	java_id integer,
	res_w integer,
	res_h integer,
	force_fullscreen boolean,
	mem_max integer,
	launch_args text,
	launch_env text,
	hook_pre text,
	hook_wrapper text,
	hook_post text,
	constraint setting_profiles_pk primary key (name),
	constraint setting_profiles_java_versions_id_fk foreign key (java_id) references java_versions(id)
);

create table clusters (
	id integer not null,
	path text not null,
	stage integer default (0) not null,
	created_at integer default (unixepoch()) not null,
	updated_at integer default (unixepoch()) not null,
	group_id integer,
	name text not null,
	mc_version text not null,
	mc_loader integer not null,
	mc_loader_version text,
	last_played integer,
	overall_played integer,
	icon_url text,
	setting_profile_name text,
	linked_pack_id text,
	linked_pack_version integer,
	constraint clusters_pk primary key (id autoincrement),
	constraint clusters_cluster_groups_id_fk foreign key (group_id) references cluster_groups(id),
	constraint clusters_setting_profiles_name_fk foreign key (setting_profile_name) references setting_profiles(name)
);

create index clusters_path_idx on clusters (path);

create table cluster_packages (
	cluster_id integer not null,
	package_hash text not null,
	constraint cluster_packages_pk primary key (cluster_id, package_hash),
	constraint cluster_packages_clusters_id_fk foreign key (cluster_id) references clusters(id),
	constraint cluster_packages_packages_hash_fk foreign key (package_hash) references packages(hash)
);

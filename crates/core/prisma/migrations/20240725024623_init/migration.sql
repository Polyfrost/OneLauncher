-- CreateTable
CREATE TABLE "settings" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT DEFAULT 0,
    "theme" TEXT NOT NULL DEFAULT 'dark',
    "hide_close_prompt" BOOLEAN NOT NULL DEFAULT true,
    "disable_animations" BOOLEAN NOT NULL DEFAULT false,
    "disable_analytics" BOOLEAN NOT NULL DEFAULT false,
    "debug_mode" BOOLEAN NOT NULL DEFAULT false,
    "hide_on_launch" BOOLEAN NOT NULL DEFAULT false,
    "force_fullscreen" BOOLEAN NOT NULL DEFAULT false,
    "disable_discord" BOOLEAN NOT NULL DEFAULT false,
    "custom_java_args" TEXT NOT NULL,
    "custom_env_args" TEXT NOT NULL,
    "max_async_io_operations" INTEGER NOT NULL DEFAULT 10,
    "max_async_fetches" INTEGER NOT NULL DEFAULT 10,
    "resolution_x" INTEGER NOT NULL DEFAULT 854,
    "resolution_y" INTEGER NOT NULL DEFAULT 480,
    "memory_max" INTEGER NOT NULL DEFAULT 2048,
    "memory_min" INTEGER NOT NULL DEFAULT 1024,
    "hook_pre" TEXT,
    "hook_wrapper" TEXT,
    "hook_post" TEXT
);

-- CreateTable
CREATE TABLE "java_version" (
    "major_version" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "full_version" TEXT NOT NULL,
    "architecture" TEXT NOT NULL,
    "path" TEXT NOT NULL
);

-- CreateTable
CREATE TABLE "minecraft_user" (
    "uuid" TEXT NOT NULL PRIMARY KEY,
    "active" BOOLEAN NOT NULL DEFAULT false,
    "username" TEXT NOT NULL,
    "access_token" TEXT NOT NULL,
    "refresh_token" TEXT NOT NULL,
    "expires" INTEGER NOT NULL
);

-- CreateTable
CREATE TABLE "minecraft_device_token" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT DEFAULT 0,
    "uuid" TEXT NOT NULL,
    "private_key" TEXT NOT NULL,
    "x" TEXT NOT NULL,
    "y" TEXT NOT NULL,
    "issue_instant" INTEGER NOT NULL,
    "not_after" INTEGER NOT NULL,
    "token" TEXT NOT NULL,
    "display_claims" TEXT NOT NULL
);

-- CreateTable
CREATE TABLE "cache" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "data_type" TEXT NOT NULL,
    "alias" TEXT,
    "data" TEXT,
    "expires" DATETIME NOT NULL,
    "created" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "cluster" (
    "path" TEXT NOT NULL PRIMARY KEY,
    "stage" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "icon_path" TEXT,
    "mc_version" TEXT NOT NULL,
    "loader" TEXT NOT NULL DEFAULT 'vanilla',
    "loader_version" TEXT DEFAULT 'stable',
    "groups" TEXT NOT NULL,
    "created_at" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "modified_at" DATETIME NOT NULL,
    "played_at" DATETIME,
    "overall_played" INTEGER NOT NULL DEFAULT 0,
    "recently_played" INTEGER NOT NULL DEFAULT 0,
    "override_java_path" TEXT,
    "override_custom_java_args" TEXT NOT NULL,
    "override_custom_env_args" TEXT NOT NULL,
    "override_memory_max" INTEGER,
    "override_memory_min" INTEGER,
    "override_force_fullscreen" INTEGER,
    "override_resolution_x" INTEGER,
    "override_resolution_y" INTEGER,
    "override_hook_pre" TEXT,
    "override_hook_wrapper" TEXT,
    "override_hook_post" TEXT
);

-- CreateTable
CREATE TABLE "process" (
    "pid" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "start_time" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "name" TEXT NOT NULL,
    "executable" TEXT NOT NULL,
    "cluster_path" TEXT NOT NULL,
    "post_exit" TEXT,
    CONSTRAINT "process_cluster_path_fkey" FOREIGN KEY ("cluster_path") REFERENCES "cluster" ("path") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateIndex
CREATE UNIQUE INDEX "minecraft_user_active_key" ON "minecraft_user"("active");

-- CreateIndex
CREATE UNIQUE INDEX "cache_data_type_alias_key" ON "cache"("data_type", "alias");

-- CreateIndex
CREATE INDEX "process_cluster_path_idx" ON "process"("cluster_path");

-- CreateIndex
CREATE UNIQUE INDEX "process_pid_key" ON "process"("pid");

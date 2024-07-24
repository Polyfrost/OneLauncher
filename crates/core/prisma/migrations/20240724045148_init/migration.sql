-- CreateTable
CREATE TABLE "settings" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT DEFAULT 0,
    "theme" TEXT NOT NULL DEFAULT 'dark',
    "hide_close_prompt" BOOLEAN NOT NULL DEFAULT true,
    "disable_animations" BOOLEAN NOT NULL DEFAULT false,
    "force_fullscreen" BOOLEAN NOT NULL DEFAULT false,
    "resolution_x" INTEGER NOT NULL DEFAULT 854,
    "resolution_y" INTEGER NOT NULL DEFAULT 480,
    "memory_max" INTEGER NOT NULL DEFAULT 2048,
    "memory_min" INTEGER NOT NULL DEFAULT 1024,
    "hook_pre" TEXT,
    "hook_wrapper" TEXT,
    "hook_post" TEXT
);

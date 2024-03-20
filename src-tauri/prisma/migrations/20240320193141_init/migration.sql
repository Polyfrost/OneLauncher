-- CreateTable
CREATE TABLE "instance" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT,
    "path" TEXT,
    "hidden" BOOLEAN,
    "time_played" DATETIME NOT NULL,
    "last_played" DATETIME NOT NULL,
    "date_created" DATETIME NOT NULL
);

-- CreateTable
CREATE TABLE "java_instance" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT,
    "path" TEXT,
    "version" TEXT,
    "arch" TEXT,
    "instance_id" INTEGER
);

-- CreateTable
CREATE TABLE "location" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT,
    "path" TEXT,
    "enabled" BOOLEAN NOT NULL DEFAULT true,
    "hidden" BOOLEAN,
    "date_added" DATETIME,
    "instance_id" INTEGER,
    CONSTRAINT "location_instance_id_fkey" FOREIGN KEY ("instance_id") REFERENCES "instance" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);

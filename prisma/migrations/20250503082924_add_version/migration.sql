/*
  Warnings:

  - Added the required column `version` to the `object_types` table without a default value. This is not possible if the table is not empty.

*/
-- AlterTable
ALTER TABLE "displays" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "object_types" ADD COLUMN     "version" BIGINT NOT NULL,
ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "objects" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "packages" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

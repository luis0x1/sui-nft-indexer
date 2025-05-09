/*
  Warnings:

  - Added the required column `bcs` to the `displays` table without a default value. This is not possible if the table is not empty.
  - Added the required column `object_type` to the `displays` table without a default value. This is not possible if the table is not empty.
  - Added the required column `version` to the `displays` table without a default value. This is not possible if the table is not empty.

*/
-- AlterTable
ALTER TABLE "displays" ADD COLUMN     "bcs" TEXT NOT NULL,
ADD COLUMN     "object_type" TEXT NOT NULL,
ADD COLUMN     "version" INTEGER NOT NULL,
ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "objects" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "packages" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();
